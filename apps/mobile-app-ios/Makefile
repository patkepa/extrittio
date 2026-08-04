SHELL := /bin/bash

.PHONY: bootstrap generate build staging release test lint format format-check module-check

bootstrap:
	./Scripts/bootstrap.sh

generate:
	./Scripts/generate-project.sh

build:
	./Scripts/build.sh

staging:
	CONFIGURATION=Staging SCHEME=Extrittio-Staging ./Scripts/build.sh

release:
	CONFIGURATION=Release SCHEME=Extrittio ./Scripts/build.sh

test:
	./Scripts/test.sh

lint:
	./Scripts/lint.sh

format:
	./Scripts/format.sh

format-check:
	./Scripts/format.sh --check

module-check:
	./Scripts/check-module-boundaries.sh
