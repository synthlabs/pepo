ROOT := $(CURDIR)
TOKE ?= toke
PNPM ?= pnpm
POWERSHELL ?= powershell

ifeq ($(OS),Windows_NT)
PATCH_CMD := $(POWERSHELL) -NoProfile -ExecutionPolicy Bypass -File "$(ROOT)/utils/scripts/patch.ps1"
else
PATCH_CMD := "$(ROOT)/utils/scripts/patch.sh"
endif

.PHONY: dev
dev:
	$(TOKE) -v $(PNPM) tauri dev

.PHONY: dev-internal
dev-internal: export ENABLE_INTERNAL := 1
dev-internal: export PEPO_LOG := debug
dev-internal:
	$(TOKE) -v $(PNPM) tauri dev

.PHONY: build
build:
	$(TOKE) -v $(PNPM) tauri build

.PHONY: build-internal
build-internal: export ENABLE_INTERNAL := 1
build-internal: export PEPO_LOG := debug
build-internal:
	$(TOKE) -v $(PNPM) tauri build

.PHONY: patch
patch:
	$(PATCH_CMD)
