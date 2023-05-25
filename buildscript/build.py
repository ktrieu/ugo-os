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
    # Handle cases where the destination folders don't exist.
    dirname = os.path.dirname(dst_path)
    if not os.path.exists(dirname):
        os.makedirs(dirname)

    shutil.copyfile(src_path, dst_path)


def copy_if_newer(src_path, dst_path):
    # If the destination doesn't exist, we definitely need to copy.
    if not os.path.exists(dst_path):
        copy_file(src_path, dst_path)

    # Otherwise, check modification times so we only copy when needed.
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


def run_cmd_in_dir(cmd, args, dir="./", suppress_stderr=False, wait=False):
    p = subprocess.Popen(
        [cmd, *args], cwd=dir, stderr=subprocess.DEVNULL if suppress_stderr else None
    )

    if wait:
        exit_val = p.wait()
        if exit_val != 0:
            raise RuntimeError(f"Command failed: {cmd} {' '.join(args)}")


# Commands
def cmd_build_bootloader():
    run_cmd_in_dir("cargo", ["build"], get_bootloader_path(), wait=True)


def cmd_build_kernel():
    run_cmd_in_dir("cargo", ["build"], get_kernel_path(), wait=True)


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

def run_qemu(debug=False):
    cmd_args = [
        "-bios",
        "ovmf/OVMF-pure-efi.fd",
        "-net",
        "none",
        "-drive",
        f"file=fat:rw:{get_bootimg_path()},format=raw",
        "-monitor",
        "stdio",
        "-D",
        "qemu.log",
        "-d",
        "int",
        "-no-reboot",
        "-action",
        "shutdown=pause"
    ]

    if debug is True:
        cmd_args.extend(["-s", "-S"])

    run_cmd_in_dir(
        "qemu-system-x86_64",
        cmd_args,
        # QEMU is giving us weird warnings about UWP, so suppress them here.
        suppress_stderr=True,
        # Don't wait and just exit immediately so we can start the debugger at the same time.
        wait=not debug
    )

def cmd_run(debug=False):
    cmd_build()
    cmd_install()
    run_qemu(debug=debug)


def usage():
    print("build.py [build|install|run|debug]")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        usage()
        sys.exit(1)

    cmd_name = sys.argv[1]

    try:
        if cmd_name == "build":
            cmd_build()
        elif cmd_name == "install":
            cmd_install()
        elif cmd_name == "run":
            cmd_run()
        elif cmd_name == "debug":
            cmd_run(debug=True)
        else:
            usage()
            sys.exit(1)
    except RuntimeError as e:
        print(e)
