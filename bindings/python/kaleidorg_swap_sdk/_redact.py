"""Keep per-swap secrets out of the generated records' ``__str__``.

UniFFI gives every record a ``__str__`` that prints all of its fields, so
``logger.info("%s", response)`` or ``print(response)`` would write the maker's
per-swap ``swap_auth`` credential, and a Liquid ``blinding_key``, into
application logs. The Rust ``Debug`` impls withhold both; the generated Python
glue does not inherit that, and the glue is regenerated on every build (by
Maturin for wheels, by ``make generate-python-bindings`` for the committed
fallback), so it cannot be edited by hand. This module patches the records at
import time instead, which covers whichever glue the package loads.

Attribute access is untouched: ``response.swap_auth`` still returns the value
so it can be persisted with the swap.
"""

import inspect

#: Record fields whose value is secret material.
SECRET_FIELDS = frozenset({"swap_auth", "blinding_key"})
REDACTED = "<redacted>"


def _record_fields(cls):
    """The keyword-only constructor parameters of a UniFFI record, in order.

    UniFFI records take every field as a keyword-only argument; objects and
    other classes do not, so they come back empty and are left alone.
    """
    try:
        parameters = inspect.signature(cls.__init__).parameters
    except (TypeError, ValueError):
        return ()
    fields = tuple(
        name
        for name, parameter in parameters.items()
        if parameter.kind is inspect.Parameter.KEYWORD_ONLY
    )
    if any(
        parameter.kind
        in (inspect.Parameter.POSITIONAL_ONLY, inspect.Parameter.POSITIONAL_OR_KEYWORD)
        for name, parameter in parameters.items()
        if name != "self"
    ):
        return ()
    return fields


def _redacting_str(cls, fields):
    """A ``__str__`` in the generated shape, with secret fields withheld.

    The generated method is ``Name(field={}, ...)`` over ``str()`` of each
    field in declaration order; this reproduces it exactly, except that a
    secret that is set prints as ``<redacted>``. ``None`` still prints as
    ``None`` — whether the maker issued a credential is worth seeing.
    """

    def __str__(self):
        parts = []
        for name in fields:
            value = getattr(self, name)
            if name in SECRET_FIELDS and value is not None:
                value = REDACTED
            parts.append(f"{name}={value}")
        return f"{cls.__name__}({', '.join(parts)})"

    __str__.__qualname__ = f"{cls.__name__}.__str__"
    __str__.__module__ = cls.__module__
    return __str__


def redact_secret_fields(namespace):
    """Replace ``__str__`` on every record in ``namespace`` with a secret field.

    ``namespace`` is the generated glue module (or a package that re-exports
    it). Records nested inside others print through their own ``__str__``, so
    patching each record class covers ``CreateChainResponse`` printing its two
    ``ChainSwapDetails`` legs as well. Returns the names of the patched classes,
    so a test can see what was covered.
    """
    patched = []
    for name, obj in list(vars(namespace).items()):
        if not inspect.isclass(obj):
            continue
        fields = _record_fields(obj)
        if not SECRET_FIELDS.intersection(fields):
            continue
        obj.__str__ = _redacting_str(obj, fields)
        patched.append(name)
    return patched
