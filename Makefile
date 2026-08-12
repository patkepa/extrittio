.PHONY: install-extrittio

# Build the embedded web UI and install the local Turso hobby appliance.
install-extrittio:
	npm --prefix apps/frontend ci
	npm --prefix apps/frontend run build
	cargo install --path apps/extrittio --locked --no-default-features --features hobby
