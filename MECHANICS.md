# Bandtools — mechanics sheet

Facts read from the original (GarageBand.app) and the values this rebuild uses.
Sources are files inside the original bundle; nothing was copied, only numbers.
"estimated" means we looked and did not find the value, so we chose it.

## Amp (from `Resources/Plug-In Settings/Amp Designer/CSParameterOrder.plist`, evidence ev_3931901b…)
- Smart-control knobs, in order: Gain, Bass, Mids, Treble, Presence, Master, Output Gain, Reverb on, Reverb level, FX on/depth/speed.
- Knob range 0–10 (observed in every preset file); Output Gain is in dB (e.g. -5.0, -7.0).
- Default preset (`#default.pst`, float block at offset 0x20): gain 4.5, bass 6.7, mids 3.1, treble 7.4, presence 8.3, master 9.4, output -7 dB, reverb level 2.2.
- Factory preset groups: Clean (48), Crunch (30), Distorted (35). Our library uses the knob values of:
  - Brit Reverb 4.4/5.1/7.0/6.2/6.4/8.4, -5 dB, reverb 8.5 → "Glass Clean"
  - Clean Jazz Fat 2.7/7.2/5.0/0/0/10, -3.5 dB, reverb 1.5 → "Warm Jazz"
  - Classic Country 4.8/5.4/5.2/7.0/6.5/7.2, -7 dB, reverb 6.5 → "Country Snap"
  - Vintage Stack 7.1/5.0/5.0/6.7/4.3/7.7, -4 dB, reverb 3.8 → "Vintage Stack Crunch"
  - Pushed Brit 7.2/7.1/5.1/10/6.1/8.3, -5 dB, reverb 4.1 → "Pushed British"
  - Raw Rhythm 10/5.6/2.7/5.6/6.9/7.9, -7.5 dB, reverb 5.0 → "Rhythm Grind"
  - Modern Lead 10/3.2/7.7/6.2/4.7/8.7, -4 dB, reverb 2.3 → "Singing Lead"
  - Modern Rock 1 6.6/5.3/4.6/4.1/3.8/5.7, -4 dB, reverb 2.3 → "Modern Rock Wall"
  - Turbo Octane 8/6.6/5.5/5.9/6.2/10, -8 dB, reverb 0 → "High Octane"
- Speaker cabinet and microphone (type + X/Z position) are separate parameters ("12 Speaker", "13 Microphone type", "14/16 Microphone pos"): we offer 8 cabinets, 3 mics and a center↔edge position.

## Pedalboard (from `Plug-In Settings/Pedalboard/CSParameterOrder.plist` and MAPlugInGUI stomp-box views)
- 14 pedal slots ("200 Slot 1 … 213 Slot 14 Stompbox Type") and 8 macro knobs (A–H). We keep the 14-slot limit; macros are not rebuilt.
- Stomp-box families in the original: compressor, noise suppressor, overdrives/distortions/fuzzes, octaver, wahs, chorus, phaser, flanger, tremolo, vibrato, rotary, delays (echo, tape, reverse), spring reverb, EQ, ring mod, whammy. We rebuild 16 of these with our own names (no brand names).

## Decompiled from the main program (GarageBand Mach-O, sha256 5a181fd9…)
Read with the built-in pseudo-C lifter (Ghidra's decompiler is not installed on this Mac, so the lifter's output is register-level and approximate).

| Rule | Function (address) | What the code does | Used in the rebuild |
|---|---|---|---|
| In-tune window ±3 cents | `-[DfChordTrainerWindowController _tunerTimerCallback:]` @0x100d8914c (ev_bd7d8fb6…) | `(unsigned)(deviation + 3) > 6` → "wrong/flat-sharp" colour, else "correct" colour | tuner turns green when the rounded deviation is within ±3 cents (`dsp::IN_TUNE_CENTS`) |
| Needle range ±50 cents | same @0x100d8914c | needle value clamped with double constants 50.0 (0x4049000000000000) and -50.0 | needle clamp `dsp::NEEDLE_CENTS` = 50 |
| Tuner output: note, cents, octave | `-[DfChannel getTunerNote:andTune:andOctave:]` @0x1008b8b04 (ev_0e8fb2e0…) | note = engine value − 1; tune = float cents converted to integer; octave from a separate call; error 0xc0200000 when no tuner is running | we show whole cents, note + octave |
| Note names | `-[DfDocument tunerStringForMidiNote:]` @0x100e047b4 (ev_541d88df…) | `midi % 12` indexes a 12-name table; a negative note returns an empty string | `dsp::note_name_in` |
| Sharp / flat spelling | `-[DfDocument _updateTunerStrings]` @0x100e04640 (ev_a689e8a0…) | builds two 12-name tables (sharps, flats) and picks one by a flag | "Flat names" switch on the tuner page |
| Tuning presets | strings `LearnToPlayTunerStrings|||Standard Tuning / Open D / Dropped D / Tune Down Whole Step` @0x1013e4883–0x1013e4918 (ev_ce21369c…) | four named tunings | tuning menu on the tuner page; it also sets the tab's tuning and the AI tab recognizer's fretboard |
| Reference pitch is per song | `CMDLogicInterface::SetGlobalTuningReferenceCB` called from sub_10019f358 @0x10019fcf4 with callback @0x1010fc3b4 (`adrp/add/ret` → global float @0x101a22f60); writers sub_1004f2f80 @0x1004f3484 and sub_100782600 @0x100782c2c copy it from song field +0x14 (ev_3ee0ffd5…, ev_34a37b03…, ev_b69c0ca4…, ev_374910494…) | the A4 reference is stored in each song, not hard-coded; no 440.0 float constant exists in the program's code (ev_3c5453b9…) | A4 stays adjustable (430–450 Hz); 440 Hz default is **estimated** |
| Tempo storage | `-[GenInstTrackModel setDefaultTempo:ifNeededForOptions:]` @0x100b37d38 (ev_88b3f98c…) | tempo is passed on as `bpm × 10000` (0x2710) fixed-point | we keep tempo as a float bpm; same resolution in practice |
| Count-in flag | `-[DfDocument isCountInEnabled]` @0x10026a1b0 (ev_65cbac02…) | reads a byte at +0xf8 of the transport settings, true when non-zero | not rebuilt (no count-in yet) |

Tuning pitches (MIDI, high e → low E): Standard 64 59 55 50 45 40; Open D 62 57 54 50 45 38; Dropped D 64 59 55 50 45 38; Tune Down Whole Step 62 57 53 48 43 38. The names come from the original; the pitches are what those names mean, not read from the code (**estimated**).

## Our own DSP formulas (not taken from the original's code)
- Amp drive = 10^((gain/10 × model max gain dB)/20); model max gain 18–56 dB; 1–3 clipping stages.
- Tone stack: bass low-shelf 120 Hz, (bass-5)×1.8 dB; mids peak at the model's mid frequency, (mids-5)×1.8 dB; treble high-shelf 2.8 kHz, (treble-5)×1.6 dB; presence high-shelf 5 kHz, (presence-5)×1.2 dB.
- Cabinet: high-pass + resonance bump + mid notch + low-pass (4.2–6.2 kHz); mic edge lowers the low-pass by up to 35 %.
- Tuner pitch detection: YIN, threshold 0.15 (ours; the original's detector is in MADSP and not decompiled). The in-tune window, needle range and note naming above are the original's.
- Tab: one text-tab column = a sixteenth note; frets 0–22; standard tuning E2 A2 D3 G3 B3 E4.

## Not decompiled
- The original's amp/cabinet DSP and the tuner's pitch detector live in MADSP.framework (49 MB), not in the main program. They were not decompiled, so all sound-shaping formulas above are our own design (see unknown unk_8c9cb44744b8a6bb).
- Stem splitting, YouTube download and AI do not exist in the original; they use open-source tools (Demucs, yt-dlp, Basic Pitch, Ollama LLMs).
