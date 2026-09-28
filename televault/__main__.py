"""Root entrypoint for TeleVault.
- Running without arguments or with 'gui' launches the Windows 11 Fluent Desktop GUI Application.
- Running with subcommands or arguments dispatches to the CLI / Web launcher.
"""

import os
import sys

if sys.stdout is None:
    sys.stdout = open(os.devnull, "w", encoding="utf-8")
if sys.stderr is None:
    sys.stderr = open(os.devnull, "w", encoding="utf-8")


def main():
    if len(sys.argv) == 1 or (len(sys.argv) > 1 and sys.argv[1] == "gui"):
        from televault.presentation.gui.app import run_gui
        sys.exit(run_gui())
    else:
        from televault.presentation.cli.__main__ import main as cli_main
        cli_main()


if __name__ == "__main__":
    main()
