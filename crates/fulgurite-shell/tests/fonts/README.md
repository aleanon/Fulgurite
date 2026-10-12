# The pinned test face

`DejaVuSans.ttf` (759,720 bytes, copied from Debian's `fonts-dejavu-core`,
`/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`), committed whole so the text tests shape a face the
repository controls instead of whatever the machine has installed
(M43/T43.11). Before, the shaping tests returned early when no system
face existed — a pass that shaped nothing — and on a machine with Noto
installed they shaped Noto, not DejaVu.

Licence: Bitstream Vera, with DejaVu's changes in the public domain;
`LICENSE-DejaVu.txt` is the upstream notice. It permits redistribution.
