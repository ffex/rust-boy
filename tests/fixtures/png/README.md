These small fixtures were constructed for rust-boy, with Python's standard-library
`struct`/`zlib`: standard PNG chunk CRCs, zlib IDAT, filter byte zero per row.
Both are 8×8 with pixel index `(x + y) % 4`.

- `adam7-rgb.png`: interlaced RGB, RGB level `255 - 85 * index`, using the seven
  Adam7 passes `(x0,y0,dx,dy)` = `(0,0,8,8)`, `(4,0,8,8)`, `(0,4,4,8)`,
  `(2,0,4,4)`, `(0,2,2,4)`, `(1,0,2,2)`, `(0,1,1,2)`.
- `gray2.png`: noninterlaced packed two-bit grayscale, sample `index` (so the
  default white-to-black palette reverses it to pixel index `3 - index`).

Expected bitplanes in the tests are hand-written byte patterns, independent of the
production tile serializer. No external artwork or licensing is involved.
