import subprocess
import shutil
from pathlib import Path

BOOTLOADER_FILE_PATH = (
    Path(__file__).parent.parent / "gb" / "src" / "components" / "bootloader.rs"
)
BIN_PATH = Path("vendor/build/bin/BootROMs/cgb_boot.bin")
DELETE_DIR = Path("vendor/build")

bytes = BIN_PATH.read_bytes()

data_str = f"pub const CGB_BOOT: [u8; {len(bytes)}] = [\n"
for index in range(0, len(bytes), 16):
    data_str += (
        ", ".join([f"0x{byte:02x}" for byte in bytes[index : index + 16]]) + ",\n"
    )

data_str += "];"

original_str = BOOTLOADER_FILE_PATH.read_text()
replace_index = original_str.find("pub const CGB_BOOT")
replace_str = original_str[:replace_index] + data_str

with open(BOOTLOADER_FILE_PATH, "w") as f:
    f.write(replace_str)

# yes, precommit does this but i want it formatted immediately
subprocess.run(f"rustfmt {BOOTLOADER_FILE_PATH}", shell=True)

if DELETE_DIR.exists():
    shutil.rmtree(BIN_PATH.parent.parent.parent)
