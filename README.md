# moonlander-rmk-1

Firmware for the ZSA Moonlander MK I keyboard, built using RMK and the Embassy async embedded Rust ecosystem,
replacing ZSA's stock QMK-based firmware.

## Features

The firmware provides a modular set of Cargo features allowing you to customize your build:

- **`rev_a` and `rev_b`**: Target configuration flags for Hardware Revision A and B boards.
- **`indicators`**: Enables support for indicator LEDs.
- **`locks`**: Automatically enables `indicators` and enables support for lock status (i.e., Num Lock, Caps Lock).
- **`storage`**: Enables persistent onboard configuration and storage via `rmk/storage`.
- **`rgb`**: Enables base underglow and lighting support. (*In progress*)
- **`rgb2`**: Advanced lighting based on `rmk-palettefx` for complex visual effects. (*In progress*)
- **`audio`**: Enables sound/audio integration. (*In progress*)
- **`halfmoon`**: Configures the firmware to discard the right side of the keyboard.
- **`debounce_fast`**: Uses a high-frequency key debouncing logic for minimal latency.
- **`vial`**: Enables host communication and live keymap configuration support with Vial.
- **`defmt`**: Enables structured, low-overhead logging and debugging across the entire Embassy ecosystem.

## Building & Flashing

> **⚠️ WARNING — HARDWARE REVISION SELECTION:**
>
> You **must** select the correct hardware revision (`rev_a` or `rev_b`) matching your Moonlander board. Flashing the wrong
revision can misalign the vector table and render the device unresponsive. Always verify your hardware revision before compiling
and flashing.

### Via Docker (Recommended)

#### Requirements

- Docker Engine installed and running.
- The Moonlander connected over USB and in DFU mode when the flashing step begins.

#### Usage

Run `flash.sh` from the repository root. The script builds the Docker image, compiles the firmware, converts the ELF executable to
a binary image, and flashes the device using `dfu-util`.

For example, to build and flash a Revision A board with Vial support:

```bash
./flash.sh --profile release --features rev_a,vial
```

For a Revision B board:

```bash
./flash.sh --profile release --features rev_b,vial
```

The `--profile` argument is optional. If omitted, Cargo uses its default development profile. The `--features` argument is also
optional at the script level, but the firmware's build script requires a valid hardware revision to be selected.

### Local

To build and flash without Docker, install the following dependencies on your host:

- **Rust toolchain:** stable Rust, managed with `rustup`.
- **Rust target:** `thumbv7em-none-eabihf`, configured in `.cargo/config.toml` and installed through `rustup`.
- **Build tools:** `build-essential`, `pkg-config`, and any additional native dependencies required by the project.
- **ARM GNU toolchain:** `gcc-arm-none-eabi` and `binutils-arm-none-eabi` (provides `arm-none-eabi-objcopy`).
- **DFU utilities:** `dfu-util`.
- **Additional Rust tools:** `flip-link`, if required by the project's linker configuration.

Build the firmware locally:

```bash
cargo build --release --features rev_a,vial
```

To flash locally, convert the generated ELF executable to a binary image and use `dfu-util` with the correct flash address for your
hardware revision. Revision A uses `0x08000000`; Revision B uses `0x08002000` because its custom bootloader occupies the first
**8 KiB** of flash.

**Always verify the hardware revision and flash address before writing firmware.**

## License

Licensed under either of Apache License 2.0 or MIT at your option.

> *This project is not affiliated with or endorsed by ZSA Technology Labs, Inc.*
