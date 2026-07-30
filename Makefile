ROOT := $(CURDIR)
TOKE ?= toke
PNPM ?= pnpm
POWERSHELL ?= powershell

ifeq ($(OS),Windows_NT)
PATCH_CMD := $(POWERSHELL) -NoProfile -ExecutionPolicy Bypass -File "$(ROOT)/utils/scripts/patch.ps1"
else
PATCH_CMD := "$(ROOT)/utils/scripts/patch.sh"
endif

TAURI_PACKAGING_PROJECT_ROOT := $(ROOT)
TAURI_PACKAGING_TOKE := $(TOKE)
TAURI_PACKAGING_PNPM := $(PNPM)
TAURI_PACKAGING_POWERSHELL := $(POWERSHELL)
TAURI_PACKAGING_PASSTHROUGH_VARS := ENABLE_INTERNAL PEPO_LOG

.PHONY: dev
dev:
	$(TOKE) -v $(PNPM) tauri dev

.PHONY: dev-internal
dev-internal: export ENABLE_INTERNAL := 1
dev-internal: export PEPO_LOG := debug
dev-internal:
	$(TOKE) -v $(PNPM) tauri dev

build-internal: export ENABLE_INTERNAL := 1
build-internal: export PEPO_LOG := debug

include utils/packaging/tauri/tauri.mk

.PHONY: format
format:
	$(TOKE) -v $(PNPM) run format

.PHONY: lint
lint:
	$(TOKE) -v $(PNPM) run lint

.PHONY: patch
patch:
	$(PATCH_CMD)
