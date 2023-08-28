PROJECT       = blobstore_vault
CAPABILITY_ID = wasmcloud:blobstore
VENDOR        = "wasmCloud"
NAME          = "Blobstore: Hashicorp Vault"
VERSION       = $(shell cargo metadata --no-deps --format-version 1 | jq -r '.packages[] .version' | head -1)
REVISION      = 0
oci_url       = localhost:5000/v2/$(PROJECT):$(VERSION)

include ./provider.mk

test::
	rustfmt --edition 2021 --check src/*.rs
	cargo clippy --all-features --all-targets
