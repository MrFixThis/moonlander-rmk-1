#!/usr/bin/env bash

set -e

IMAGE_NAME="rmk-moonlander-flasher"
PROFILE=""
FEATURES=""
REVISION=""
NO_DEFAULT_FEATURES=false

if ! command -v docker &> /dev/null || ! docker info &> /dev/null; then
    echo "Error: Docker engine is required but not running or installed." >&2
    exit 1
fi

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --profile)
            if [[ "$#" -lt 2 || -z "$2" ]]; then
                echo "Error: --profile requires a value." >&2
                exit 1
            fi
            PROFILE="$2"
            shift
            ;;
        --features)
            if [[ "$#" -lt 2 || -z "$2" ]]; then
                echo "Error: --features requires a value." >&2
                exit 1
            fi
            FEATURES="$2"
            shift
            ;;
        --revision)
            if [[ "$#" -lt 2 || -z "$2" ]]; then
                echo "Error: --revision requires a value." >&2
                exit 1
            fi
            REVISION="$2"
            shift
            ;;
        --no-default-features)
            NO_DEFAULT_FEATURES=true
            ;;
        *)
            echo "Unknown parameter: $1" >&2
            exit 1
            ;;
    esac
    shift
done

case "$REVISION" in
    rev-a|rev-b) ;;
    *)
        echo "Error: --revision must be specified (rev-a or rev-b)." >&2
        exit 1
        ;;
esac

echo "==> Building Docker image..."
docker build -t "$IMAGE_NAME" .

echo "==> Running compilation and flashing inside ephemeral container..."
docker run --rm --privileged \
    -v /dev/bus/usb:/dev/bus/usb \
    -v "$(pwd)":/workspace \
    -w /workspace \
    -e PROFILE="$PROFILE" \
    -e FEATURES="$FEATURES" \
    -e REVISION="$REVISION" \
    -e NO_DEFAULT_FEATURES="$NO_DEFAULT_FEATURES" \
    "$IMAGE_NAME" bash -c '
        set -e

        BUILD_ARGS=()

        if [[ -n "$PROFILE" ]]; then
            BUILD_ARGS+=(--profile "$PROFILE")
        fi

        if [[ -n "$FEATURES" ]]; then
            BUILD_ARGS+=(--features "$FEATURES")
        fi

        if [[ "$NO_DEFAULT_FEATURES" == "true" ]]; then
            BUILD_ARGS+=(--no-default-features)
        fi

        echo "==> Revision: $REVISION"
        echo "==> Compiling firmware..."

        cargo build "${BUILD_ARGS[@]}"

        # Cargo uses "debug" when no profile is specified.
        OUT_DIR="${PROFILE:-debug}"
        ELF_PATH="target/thumbv7em-none-eabihf/$OUT_DIR/moonlander-rmk-1"
        BIN_PATH="${ELF_PATH}.bin"

        if [[ ! -f "$ELF_PATH" ]]; then
            echo "Error: ELF not found: $ELF_PATH" >&2
            exit 1
        fi

        echo "==> Converting ELF to binary..."
        arm-none-eabi-objcopy -O binary "$ELF_PATH" "$BIN_PATH"

        case "$REVISION" in
            rev-a) FLASH_ADDR="0x08000000" ;;
            rev-b) FLASH_ADDR="0x08002000" ;;
        esac

        echo "==> Firmware: $BIN_PATH"
        echo "==> Flash address: $FLASH_ADDR"
        echo "==> Flashing device via dfu-util..."

        dfu-util -R -D "$BIN_PATH" -a 0 -s "${FLASH_ADDR}:leave"
    '

echo "==> Done!"
