PROFILE = dev

build: \
	$(addprefix public/assets/,$(notdir $(wildcard src/static/assets/*))) \
	public/index.html \
	public/main.css \
	public/papirs_client.js \
	public/papirs_client_bg.wasm

public/assets/%: src/static/assets/%
	@mkdir -p public/assets
	cp $< $@

public/%.html: src/static/%.html
	@mkdir -p public
	cp $< $@

public/%.css: src/static/%.sass
	@mkdir -p public
	sass --no-source-map $< $@

public/papirs_client.js public/papirs_client_bg.wasm: package
package:
	@mkdir -p public
	wasm-pack build --no-typescript --target web --$(PROFILE)
	cp pkg/papirs_client.js pkg/papirs_client_bg.wasm public

clean:
	rm -rf pkg public
	cargo clean

.PHONY: build package clean
