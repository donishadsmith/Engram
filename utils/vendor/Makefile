# Modified from Sameboy, by deleting things not pertaining to the logo and adding a python script.
NATIVE_CC := cc
PB12_COMPRESS := build/pb12
BIN := build/bin
OBJ := build/obj
MKDIR := mkdir

RGBASM  := $(RGBDS)rgbasm
RGBLINK := $(RGBDS)rgblink
RGBGFX  := $(RGBDS)rgbgfx

RGBASM_FLAGS := $(if $(filter $(shell echo 'println __RGBDS_MAJOR__ || (!__RGBDS_MAJOR__ && __RGBDS_MINOR__ > 6)' | $(RGBASM) -), $$0), -h,) --include $(OBJ)/BootROMs/ --include BootROMs/

PYTHON := python3
RGBGFX_FLAGS := -Z -c '\#FFFFFF,\#7F7F7F,\#FF0000,\#000000'

bootroms: $(BIN)/BootROMs/cgb_boot.bin
	cd .. && $(PYTHON) update_bootloader.py

$(OBJ)/BootROMs/Engram.2bpp: ../engram_logo.png
	-@$(MKDIR) -p $(dir $@)
	$(RGBGFX) $(RGBGFX_FLAGS) -o $@ $<

$(OBJ)/BootROMs/Engram.pb12: $(OBJ)/BootROMs/Engram.2bpp $(PB12_COMPRESS)
	-@$(MKDIR) -p $(dir $@)
	"$(realpath $(PB12_COMPRESS))" < $< > $@

$(PB12_COMPRESS): BootROMs/pb12.c
	-@$(MKDIR) -p $(dir $@)
	$(NATIVE_CC) -std=c99 -Wall -Werror $< -o $@


$(BIN)/BootROMs/%.bin: BootROMs/%.asm $(OBJ)/BootROMs/Engram.pb12
	-@$(MKDIR) -p $(dir $@)
	$(RGBASM) $(RGBASM_FLAGS) -o $@.tmp $<
	$(RGBLINK) -x -o $@ $@.tmp
	@rm $@.tmp

.PHONY: bootroms
