# Media playback

Android inline previews use bundled LibVLC, the same decoder as the fullscreen
compatibility player. Its native view follows the preview area and pauses when
hidden or backgrounded; it is released when the selection closes or changes.
Other platforms use the host WebView for inline preview. Container extensions
do not identify the codecs or H.264 profile/bit depth inside a file; an MP4 can
still exceed a particular decoder's capabilities.

Compatibility options in the selected-file preview:

- Android: **Open compatibility player** uses bundled LibVLC in an in-app player
  with play/pause, seeking and close controls. Inline playback has play/pause
  and seeking in the preview area. Local and paired media use the
  same permission-checked sources. Remote HTTPS remains validated by the native
  pinned-certificate gateway; the decoder connects only to an ephemeral loopback
  proxy with an opaque, session-specific path.
- iOS local files: **Open compatibility player** uses bundled VLCKit while holding
  the Files security scope. Playback pauses when the app enters the background.
- Windows, macOS and Linux: **Create compatibility preview** converts the first
  30 seconds into a temporary H.264/AAC MP4. FFmpeg must be installed on the library
  computer and available on PATH, or selected with `MEDIA_MANAGER_FFMPEG`.
- Paired iOS: compatibility conversion runs on the library computer, then the
  bounded preview is downloaded using the paired TLS configuration and opened in
  the native player. It does not send the media to a third-party service.

Conversion runs outside the UI thread, allows one job at a time, caps execution
at 60 seconds and output at 64 MiB, and leaves original media unchanged. Temporary
local previews are retained only while their player owns them. Server previews
expire with their playback ticket (15 minutes); expired entries are removed when
new playback tickets are requested. Conversion does not grant arbitrary file
access or enable network input protocols.

Recognized containers include MP4/M4V/F4V, MKV, AVI/DivX, MOV, WebM, MPEG-TS/MTS,
MPEG/VOB, WMV/ASF, FLV, Ogg video, 3GP/3G2 and RealMedia. This is library recognition,
not a guarantee that every codec/profile, damaged file or protected stream can play.

The iOS decoder is VideoLAN's checksum-pinned 2026-09-29 VLCKit 4.0 development
binary, matching its official Swift package distribution; device acceptance is
required before shipping to devices. Third-party license/source/relinking
obligations apply to decoder-containing release packages. See THIRD_PARTY_NOTICES.md.

Developer decoding check (debug Android APK only):

```powershell
python scripts/android_playback_smoke.py --adb .tools/android-sdk/platform-tools/adb.exe --evidence .tools/codec-validation/decode-evidence
python scripts/android_playback_smoke.py --adb .tools/android-sdk/platform-tools/adb.exe --inline --codec h264 --evidence .tools/inline-validation/h264
python scripts/android_playback_smoke.py --adb .tools/android-sdk/platform-tools/adb.exe --inline --codec h264-10bit --evidence .tools/inline-validation/h264-10bit
```

This uses FFmpeg to generate private MP4 fixtures with MPEG-4 or H.264 video
and AAC audio at 24 fps, waits for native
playing/video-output events and checks that the app remains running. It does not
validate every codec, physical-device performance or the paired HTTPS path.

For Flatpak, FFmpeg must be available inside the application sandbox; installing
it only on the host does not provide the executable inside an existing Flatpak.
The conversion fallback is not yet a self-contained runtime in release packages.
