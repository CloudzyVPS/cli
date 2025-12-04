# Zyffiliate

Generate binary (production):

1. Build a release binary:

	```bash
	cargo build --release
	```

2. The produced binary is located at `target/release/zyffiliate`.

Run the server (serve command):

1. Use the built binary and the `serve` subcommand:

	```bash
	./target/release/zyffiliate serve --host 0.0.0.0 --port 5000 --env-file /path/to/.env
	```

2. For development, you can run through cargo directly:

	```bash
	cargo run -- serve --host 127.0.0.1 --port 5000
	```

Only the binary generation and `serve` command are documented here. For additional commands and configuration, consult the source or CLI help.
