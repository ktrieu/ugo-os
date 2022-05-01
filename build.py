# Ideally this would be a shell script or some such thing, but I use Windows, so
# Python's probably the best cross-platform option.
import argparse
import sys
import os
import subprocess
import shutil

CURRENT_DIR = sys.path[0]

BOOTLOADER_PATH = os.path.join(CURRENT_DIR, "bootloader")
BOOTLOADER_OUTPUT_PATH = os.path.join(
    BOOTLOADER_PATH, "target/x86_64-unknown-uefi/debug/ugo-os.efi"
)

KERNEL_PATH = os.path.join(CURRENT_DIR, "kernel")
KERNEL_OUTPUT_PATH = os.path.join(KERNEL_PATH, "target/kernel/debug/kernel")

BOOT_IMAGE_PATH = os.path.join(CURRENT_DIR, "bootimg")
BOOT_IMAGE_BOOT_FILE = os.path.join(BOOT_IMAGE_PATH, "EFI/BOOT/BOOTX64.efi")
BOOT_IMAGE_KERNEL_FILE = os.path.join(BOOT_IMAGE_PATH, "ugo-os.elf")

QEMU_EXECUTABLE = "qemu-system-x86_64.exe"
UEFI_BIOS_PATH = os.path.join(CURRENT_DIR, "ovmf/OVMF-pure-efi.fd")


def build():
    # Build the bootloader
    subprocess.call("cargo build", cwd=BOOTLOADER_PATH, shell=True)
    # Build the kernel
    subprocess.call("cargo build", cwd=KERNEL_PATH, shell=True)

    # Assemble the boot image
    # Ensure our boot image folder actually exists
    os.makedirs(BOOT_IMAGE_PATH, exist_ok=True)
    # And copy over the bootloader file
    shutil.copy2(BOOTLOADER_OUTPUT_PATH, BOOT_IMAGE_BOOT_FILE)
    # And copy over the kernel file
    shutil.copy2(KERNEL_OUTPUT_PATH, BOOT_IMAGE_KERNEL_FILE)


def run():
    # Ensure we're all built first
    build()
    # Run QEMU with the correct args
    if not os.path.exists(UEFI_BIOS_PATH):
        print(
            f"No UEFI firmware image found at {UEFI_BIOS_PATH}. Find and download an OVMF image and place it at that location."
        )

    subprocess.call(
        f"{QEMU_EXECUTABLE} -bios {UEFI_BIOS_PATH} -net none -drive file=fat:rw:{BOOT_IMAGE_PATH},format=raw"
    )


parser = argparse.ArgumentParser(prog="build.py")
subparsers = parser.add_subparsers()

build_subparser = subparsers.add_parser("build", help="Build the OS.")
build_subparser.set_defaults(func=build)

run_subparser = subparsers.add_parser("run", help="Run the OS in QEMU.")
run_subparser.set_defaults(func=run)

if __name__ == "__main__":
    args = parser.parse_args()
    args.func()
