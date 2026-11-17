//! Off-screen drawing for `--screenshot` when macOS won't draw the window
//! (screen locked or window fully covered): runs the same UI into a GPU
//! texture and saves it as a PNG.

use eframe::egui_wgpu;
use eframe::wgpu;

pub fn render(size: [u32; 2], ppp: f32, frames: usize, mut draw: impl FnMut(&mut egui::Ui), ctx: &egui::Context) -> Result<egui::ColorImage, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).map_err(|e| e.to_string())?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&device, format, egui_wgpu::RendererOptions::default());

    let points = egui::vec2(size[0] as f32 / ppp, size[1] as f32 / ppp);
    let mut textures = egui::TexturesDelta::default();
    let mut last = None;
    for _ in 0..frames {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, points)),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| draw(ui));
        textures.append(std::mem::take(&mut out.textures_delta));
        last = Some(out);
    }
    for (id, deltas) in &textures.set {
        for delta in deltas.iter() {
            renderer.update_texture(&device, &queue, *id, delta);
        }
    }
    textures.clear();
    let out = last.ok_or("no frame")?;
    let jobs = ctx.tessellate(out.shapes, ppp);
    let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: ppp };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("shot") });
    let extra = renderer.update_buffers(&device, &queue, &mut enc, &jobs, &screen);
    {
        let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shot"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.12, g: 0.12, b: 0.13, a: 1.0 }), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        let mut pass = pass.forget_lifetime();
        renderer.render(&mut pass, &jobs, &screen);
    }
    let row = size[0] * 4;
    let padded = row.div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("read"), size: (padded * size[1]) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(size[1]) } },
        wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
    );
    queue.submit(extra.into_iter().chain(std::iter::once(enc.finish())));
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
    let data = slice.get_mapped_range().map_err(|e| format!("{e:?}"))?;
    let mut pixels = Vec::with_capacity((size[0] * size[1]) as usize);
    for y in 0..size[1] {
        let start = (y * padded) as usize;
        for x in 0..size[0] as usize {
            let p = &data[start + x * 4..start + x * 4 + 4];
            pixels.push(egui::Color32::from_rgb(p[0], p[1], p[2]));
        }
    }
    Ok(egui::ColorImage { size: [size[0] as usize, size[1] as usize], pixels, source_size: egui::vec2(size[0] as f32, size[1] as f32) })
}
