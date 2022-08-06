build-bootloader:
	cd bootloader && cargo build

build-kernel:
	cd kernel && cargo build

install-bootloader: build-bootloader
	cp bootloader/target/x86_64-unknown-uefi/debug/bootloader.efi bootimg/EFI/BOOT/BOOTX64.efi

install-kernel: build-kernel
	cp kernel/target/kernel/debug/kernel bootimg/ugo-os.elf

build: install-bootloader install-kernel

run: build
	qemu-system-x86_64.exe -bios ovmf/OVMF-pure-efi.fd -net none -drive file=fat:rw:bootimg/,format=raw