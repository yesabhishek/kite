# Local renderer fix

Based on iced_tiny_skia 0.14.1, MIT licensed.

Editor text always applies the intersection of its viewport and layer clip mask.
The upstream fast path compares the editor viewport size with the clipping bounds
and skips masking when that rectangle fits. With unwrapped source lines, actual
glyphs may extend beyond that rectangle and paint over adjacent panes.

The native preview render regression changes only the source content and compares
pixels in the adjacent preview pane to verify isolation.
