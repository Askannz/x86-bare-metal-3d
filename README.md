# x86-bare-metal-3d

Demo of a software renderer running on bare-metal x86 (no operating system), rendering to a VGA text buffer. Anti-aliasing is emulated using dithering characters of the cp437 charset (░, ▒ and █
).

`cargo run` automatically executes Qemu with the produced boot image (you'll need to install Qemu x86_64).


https://user-images.githubusercontent.com/9202863/128333051-4bc7eab9-6bb8-4bb1-8ce0-507d0b7f7cc8.mp4


