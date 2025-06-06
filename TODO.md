## TODOs

- (serve) use `serde` to generate json (get rid of `map_to_entry`)

- (serve) replace `warp` with `axum` - done

- Refactor the instance API. Instead of going from the more general `BufReader`
  to the particular &[u8] type, invert the logic. Accept a &[u8] which converts
  to a `BufReader` and use the `BufReader` in the parsing logic. This makes the
  logic more complicated but will improve performance.

- Cache the attribute already read in the instance!

- `next_attribute` iterate over the top level attribute of an instance but
  should also iterate inside sequences.

- Use a enum as VR for instance::DicomAttribute struct.

- Embed a default config file in scan/server.

- Add a logfile to scan logging each files being scanned and the time taken.

- Replace structopt by clap
