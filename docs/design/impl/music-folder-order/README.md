# Music-folder order controls

The configured root order controls scan/list presentation and is already
durable data, but changing it required hand-editing `config.toml`. Settings now
puts Up/Down word controls beside each root's existing Remove action.

Only an adjacent in-range swap is accepted. The first Up and last Down are
disabled; every move is disabled while a scan is using a snapshot of the old
order or while a row's removal confirmation stands. A successful move writes
the complete ordered `music_dirs` list and starts no scan. It never moves files
or changes existing indexed rows; later scan work naturally receives the new
root order.

The established word-control grammar was chosen over introducing the product's
first folder drag handle for a normally short list. A pure regression pins both
directions and every boundary, while the all-feature check covers view/message
wiring and config persistence remains covered by the existing round trips.
