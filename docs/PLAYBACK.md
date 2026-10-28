# Media playback

The default inline preview uses the host WebView. Container extensions do not
identify the video/audio codecs inside a file; an MP4 can still be unsupported.

Compatibility options in the selected-file preview:

- Android: **Open compatibility player** uses bundled LibVLC in an in-app player
  with play/pause, seeking and close controls. Local and paired media use the
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
required before shipping. Third-party license/source/relinking obligations must
be fulfilled for decoder-containing release packages. No such release has been
published by this change. See THIRD_PARTY_NOTICES.md.
