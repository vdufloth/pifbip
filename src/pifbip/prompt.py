from prompt_toolkit import prompt
from prompt_toolkit.completion import FuzzyWordCompleter


def ask_destination(existing_dirs: list[str]) -> str:
    """Prompt user for a destination subfolder with fuzzy autocomplete."""
    completer = FuzzyWordCompleter(existing_dirs)
    result = prompt(
        "Move to folder (empty=skip): ",
        completer=completer,
        complete_while_typing=True,
    )
    return result.strip()
