import os
import shutil
import subprocess
import sys

# Paths
BOOTIMG_PATH = "bootimg/"
BOOTLOADER_PATH = "bootloader/"
KERNEL_PATH = "kernel/"

# Support functions to make the actual commands more declarative:
def copy_file(src_path, dst_path):
    shutil.copyfile(src_path, dst_path)


def copy_if_newer(src_path, dst_path):
    src_mtime = os.path.getmtime(src_path)
    dst_mtime = os.path.getmtime(dst_path)

    if src_mtime > dst_mtime:
        copy_file(src_path, dst_path)


def get_bootimg_path(path=""):
    return os.path.join(BOOTIMG_PATH, path)


def get_bootloader_path(path=""):
    return os.path.join(BOOTLOADER_PATH, path)


def get_kernel_path(path=""):
    return os.path.join(KERNEL_PATH, path)


def run_cmd_in_dir(cmd, args, dir="./"):
    p = subprocess.Popen([cmd, *args], cwd=dir)
    exit_val = p.wait()
    return exit_val == 0


# Commands
def cmd_build_bootloader():
    run_cmd_in_dir("cargo", ["build"], get_bootloader_path())


def cmd_build_kernel():
    run_cmd_in_dir("cargo", ["build"], get_kernel_path())


def cmd_build():
    cmd_build_bootloader()
    cmd_build_kernel()


def cmd_install_bootloader():
    cmd_build_bootloader()
    copy_if_newer(
        get_bootloader_path("target/x86_64-unknown-uefi/debug/bootloader.efi"),
        get_bootimg_path("EFI/BOOT/BOOTX64.efi"),
    )


def cmd_install_kernel():
    cmd_build_kernel()
    copy_if_newer(
        get_kernel_path("target/kernel/debug/kernel"),
        get_bootimg_path("ugo-os.elf"),
    )


def cmd_install():
    cmd_install_bootloader()
    cmd_install_kernel()


def cmd_run():
    cmd_build()
    cmd_install()
    run_cmd_in_dir(
        "qemu-system-x86_64.exe",
        [
            "-bios",
            "ovmf/OVMF-pure-efi.fd",
            "-net",
            "none",
            "-drive",
            f"file=fat:rw:{get_bootimg_path()},format=raw",
        ],
    )


def usage():
    print("build.py [build|install|run]")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        usage()
        sys.exit(1)

    cmd_name = sys.argv[1]

    if cmd_name == "build":
        cmd_build()
    elif cmd_name == "install":
        cmd_install()
    elif cmd_name == "run":
        cmd_run()
    else:
        usage()
        sys.exit(1)
