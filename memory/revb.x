/*
 * ZSA Moonlander revB
 *
 * Source of truth: ZSA's own qmk_firmware fork,
 * keyboards/zsa/moonlander/revb/ld/moonlander.ld
 *
 *   flash0 (rx) : org = 0x08002000, len = 128k - 0x2000
 *   ram0   (wx) : org = 0x20000000, len = 32k
 *   ram4   (wx) : org = 0x10000000, len = 8k   (CCM)
 *
 * Boot mechanism: custom bootloader occupying the first 8K of
 * flash (0x08000000 - 0x08001FFF). Application
 * code must start at 0x08002000, or the bootloader gets
 * overwritten on first flash and the board can only be recovered
 * via SWD/JTAG.
 *
 * Last 10K reserved for RMK's storage (Vial keymap persistence
 * via sequential-storage)
 */
MEMORY
{
    FLASH : ORIGIN = 0x08002000, LENGTH = 128K - 8K - 10K
    RAM   : ORIGIN = 0x20000000, LENGTH = 32K
    /* CCMRAM : ORIGIN = 0x10000000, LENGTH = 8K */
}
