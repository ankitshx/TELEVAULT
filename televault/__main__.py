"""Root entrypoint for TeleVault.
- Running without arguments launches the Windows 11 Fluent Desktop GUI Application.
- Running with subcommands (e.g. backup, restore, doctor, web) dispatches to the CLI/Web.
"""

import sys


def main():
    cli_commands = {
        "login", "logout", "web", "dashboard", "backup", "restore",
        "ls", "verify", "rebuild", "snapshot", "doctor", "recovery-test",
        "manifest", "audit", "--help", "-h"
    }

    if len(sys.argv) > 1 and sys.argv[1] in cli_commands:
        from televault.presentation.cli.__main__ import main as cli_main
        cli_main()
    else:
        from televault.presentation.gui.app import run_gui
        run_gui()


if __name__ == "__main__":
    main()
