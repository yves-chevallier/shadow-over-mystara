# Shadow over Mystara, Rust port.
#   make assets   extract the assets from the ROMs (rom/, or ddsom.zip + qsound.zip at the root)
#   make build    build the game
#   make play     build, then play from the assets
CARGO    = cargo
MANIFEST = --manifest-path port/Cargo.toml
ROMS    ?= rom
ASSETS  ?= assets
GAME     = port/target/release/ddsom

.PHONY: assets build play rom

rom: $(ROMS)/ddsom.key $(ROMS)/dl-1425.bin

$(ROMS)/ddsom.key:
	unzip -oq ddsom.zip -d $(ROMS)

$(ROMS)/dl-1425.bin:
	unzip -oq qsound.zip -d $(ROMS)

assets: rom
	$(CARGO) run --release $(MANIFEST) -p extract -- $(ROMS) $(ASSETS)

build:
	$(CARGO) build --release $(MANIFEST) -p ddsom

play: build
	$(GAME) --assets $(ASSETS)
