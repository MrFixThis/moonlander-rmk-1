/*
 * ZSA Moonlander revA
 *
 * Source of truth: ZSA's own qmk_firmware fork,
 * keyboards/zsa/moonlander/reva/ld/STM32F303xB.ld

 *   flash0 (rx) : org = 0x08000000, len = 128k
 *   ram0   (wx) : org = 0x20000000, len = 32k
 *   ram4   (wx) : org = 0x10000000, len = 8k   (CCM)
 *
 * Boot mechanism: native STM32 ROM DFU bootloader (lives outside
 * user flash, in system memory). No reserved region at the start
 * of flash.
 *
 * Last 10K reserved for RMK's storage (Vial keymap persistence
 * via sequential-storage).
 */
MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 128K - 10K
    RAM   : ORIGIN = 0x20000000, LENGTH = 32K
    /* CCMRAM : ORIGIN = 0x10000000, LENGTH = 8K */
}
