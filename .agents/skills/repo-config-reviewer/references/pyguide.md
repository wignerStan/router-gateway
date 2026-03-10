## [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#1-Background)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#1-background)1 Background

Python is the main dynamic language used at Google. This style guide is a list of _dos and don'ts_ for Python programs.

To help you format code correctly, we've created a [settings file for Vim](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/google_python_style.vim). For Emacs, the default settings should be fine.

Many teams use the [yapf](https://github.com/google/yapf/) auto-formatter to avoid arguing over formatting.

## [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2-Python-Language-Rules)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2-python-language-rules)2 Python Language Rules

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1-Lint)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1-lint)2.1 Lint

Run `pylint` over your code.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_1-definition)2.1.1 Definition

`pylint` is a tool for finding bugs and style problems in Python source code. It finds problems that are typically caught by a compiler for less dynamic languages like C and C++. Because of the dynamic nature of Python, some warnings may be incorrect; however, spurious warnings should be fairly infrequent.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_2-pros)2.1.2 Pros

Catches easy-to-miss errors like typos, using-vars-before-assignment, etc.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_3-cons)2.1.3 Cons

`pylint` isn‘t perfect. To take advantage of it, we’ll need to sometimes: a) Write around it b) Suppress its warnings or c) Improve it.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_1_4-decision)2.1.4 Decision

Make sure you run `pylint` on your code.

Suppress warnings if they are inappropriate so that other issues are not hidden. To suppress warnings, you can set a line-level comment:

```
<span>dict </span><span>=</span><span> </span><span>'something awful'</span><span>  </span><span># Bad Idea... pylint: disable=redefined-builtin</span><span>
</span>
```

`pylint` warnings are each identified by symbolic name (`empty-docstring`) Google-specific warnings start with `g-`.

If the reason for the suppression is not clear from the symbolic name, add an explanation.

Suppressing in this way has the advantage that we can easily search for suppressions and revisit them.

You can get a list of `pylint` warnings by doing:

```
<span>pylint </span><span>--</span><span>list</span><span>-</span><span>msgs
</span>
```

To get more information on a particular message, use:

```
<span>pylint </span><span>--</span><span>help</span><span>-</span><span>msg</span><span>=</span><span>C6409
</span>
```

Prefer `pylint: disable` to the deprecated older form `pylint: disable-msg`.

Unused argument warnings can be suppressed by deleting the variables at the beginning of the function. Always include a comment explaining why you are deleting it. “Unused.” is sufficient. For example:

```
<span>def</span><span> viking_cafe_order</span><span>(</span><span>spam</span><span>,</span><span> beans</span><span>,</span><span> eggs</span><span>=</span><span>None</span><span>):</span><span>
    </span><span>del</span><span> beans</span><span>,</span><span> eggs  </span><span># Unused by vikings.</span><span>
    </span><span>return</span><span> spam </span><span>+</span><span> spam </span><span>+</span><span> spam
</span>
```

Other common forms of suppressing this warning include using ‘`_`’ as the identifier for the unused argument, prefixing the argument name with ‘`unused_`’, or assigning them to ‘`_`’. These forms are allowed but no longer encouraged. The first two break callers that pass arguments by name, while the last does not enforce that the arguments are actually unused.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2-Imports)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2-imports)2.2 Imports

Use `import` statements for packages and modules only, not for individual classes or functions. Note that there is an explicit exemption for imports from the [typing module](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-imports).

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_1-definition)2.2.1 Definition

Reusability mechanism for sharing code from one module to another.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_2-pros)2.2.2 Pros

The namespace management convention is simple. The source of each identifier is indicated in a consistent way; `x.Obj` says that object `Obj` is defined in module `x`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_3-cons)2.2.3 Cons

Module names can still collide. Some module names are inconveniently long.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_2_4-decision)2.2.4 Decision

-   Use `import x` for importing packages and modules.
-   Use `from x import y` where `x` is the package prefix and `y` is the module name with no prefix.
-   Use `from x import y as z` if two modules named `y` are to be imported or if `y` is an inconveniently long name.
-   Use `import y as z` only when `z` is a standard abbreviation (e.g., `np` for `numpy`).

For example the module `sound.effects.echo` may be imported as follows:

```
<span>from</span><span> sound</span><span>.</span><span>effects </span><span>import</span><span> echo
</span><span>...</span><span>
echo</span><span>.</span><span>EchoFilter</span><span>(</span><span>input</span><span>,</span><span> output</span><span>,</span><span> delay</span><span>=</span><span>0.7</span><span>,</span><span> atten</span><span>=</span><span>4</span><span>)</span><span>
</span>
```

Do not use relative names in imports. Even if the module is in the same package, use the full package name. This helps prevent unintentionally importing a package twice.

Imports from the [typing module](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-imports) and the [six.moves module](https://six.readthedocs.io/#module-six.moves) are exempt from this rule.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3-Packages)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3-packages)2.3 Packages

Import each module using the full pathname location of the module.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3_1-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3_1-pros)2.3.1 Pros

Avoids conflicts in module names or incorrect imports due to the module search path not being what the author expected. Makes it easier to find modules.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3_2-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3_2-cons)2.3.2 Cons

Makes it harder to deploy code because you have to replicate the package hierarchy. Not really a problem with modern deployment mechanisms.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3_3-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_3_3-decision)2.3.3 Decision

All new code should import each module by its full package name.

Imports should be as follows:

Yes:

```
<span># Reference absl.flags in code with the complete name (verbose).</span><span>
</span><span>import</span><span> absl</span><span>.</span><span>flags
</span><span>from</span><span> doctor</span><span>.</span><span>who </span><span>import</span><span> jodie

FLAGS </span><span>=</span><span> absl</span><span>.</span><span>flags</span><span>.</span><span>FLAGS
</span>
```

```
<span># Reference flags in code with just the module name (common).</span><span>
</span><span>from</span><span> absl </span><span>import</span><span> flags
</span><span>from</span><span> doctor</span><span>.</span><span>who </span><span>import</span><span> jodie

FLAGS </span><span>=</span><span> flags</span><span>.</span><span>FLAGS
</span>
```

No: _(assume this file lives in `doctor/who/` where `jodie.py` also exists)_

```
<span># Unclear what module the author wanted and what will be imported.  The actual</span><span>
</span><span># import behavior depends on external factors controlling sys.path.</span><span>
</span><span># Which possible jodie module did the author intend to import?</span><span>
</span><span>import</span><span> jodie
</span>
```

The directory the main binary is located in should not be assumed to be in `sys.path` despite that happening in some environments. This being the case, code should assume that `import jodie` refers to a third party or top level package named `jodie`, not a local `jodie.py`.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4-Exceptions)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4-exceptions)2.4 Exceptions

Exceptions are allowed but must be used carefully.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_1-definition)2.4.1 Definition

Exceptions are a means of breaking out of the normal flow of control of a code block to handle errors or other exceptional conditions.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_2-pros)2.4.2 Pros

The control flow of normal operation code is not cluttered by error-handling code. It also allows the control flow to skip multiple frames when a certain condition occurs, e.g., returning from N nested functions in one step instead of having to carry-through error codes.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_3-cons)2.4.3 Cons

May cause the control flow to be confusing. Easy to miss error cases when making library calls.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_4_4-decision)2.4.4 Decision

Exceptions must follow certain conditions:

-   Raise exceptions like this: `raise MyError('Error message')` or `raise MyError()`. Do not use the two-argument form (`raise MyError, 'Error message'`).

-   Make use of built-in exception classes when it makes sense. For example, raise a `ValueError` to indicate a programming mistake like a violated precondition (such as if you were passed a negative number but required a positive one). Do not use `assert` statements for validating argument values of a public API. `assert` is used to ensure internal correctness, not to enforce correct usage nor to indicate that some unexpected event occurred. If an exception is desired in the latter cases, use a raise statement. For example:

    ```
    <span>Yes</span><span>:</span><span>
      </span><span>def</span><span> connect_to_next_port</span><span>(</span><span>self</span><span>,</span><span> minimum</span><span>):</span><span>
        </span><span>"""Connects to the next available port.

        Args:
          minimum: A port value greater or equal to 1024.

        Returns:
          The new minimum port.

        Raises:
          ConnectionError: If no available port is found.
        """</span><span>
        </span><span>if</span><span> minimum </span><span>&lt;</span><span> </span><span>1024</span><span>:</span><span>
          </span><span># Note that this raising of ValueError is not mentioned in the doc</span><span>
          </span><span># string's "Raises:" section because it is not appropriate to</span><span>
          </span><span># guarantee this specific behavioral reaction to API misuse.</span><span>
          </span><span>raise</span><span> </span><span>ValueError</span><span>(</span><span>'Minimum port must be at least 1024, not %d.'</span><span> </span><span>%</span><span> </span><span>(</span><span>minimum</span><span>,))</span><span>
        port </span><span>=</span><span> self</span><span>.</span><span>_find_next_open_port</span><span>(</span><span>minimum</span><span>)</span><span>
        </span><span>if</span><span> </span><span>not</span><span> port</span><span>:</span><span>
          </span><span>raise</span><span> </span><span>ConnectionError</span><span>(</span><span>'Could not connect to service on %d or higher.'</span><span> </span><span>%</span><span> </span><span>(</span><span>minimum</span><span>,))</span><span>
        </span><span>assert</span><span> port </span><span>&gt;=</span><span> minimum</span><span>,</span><span> </span><span>'Unexpected port %d when minimum was %d.'</span><span> </span><span>%</span><span> </span><span>(</span><span>port</span><span>,</span><span> minimum</span><span>)</span><span>
        </span><span>return</span><span> port
    </span>
    ```

    ```
    <span>No</span><span>:</span><span>
      </span><span>def</span><span> connect_to_next_port</span><span>(</span><span>self</span><span>,</span><span> minimum</span><span>):</span><span>
        </span><span>"""Connects to the next available port.

        Args:
          minimum: A port value greater or equal to 1024.

        Returns:
          The new minimum port.
        """</span><span>
        </span><span>assert</span><span> minimum </span><span>&gt;=</span><span> </span><span>1024</span><span>,</span><span> </span><span>'Minimum port must be at least 1024.'</span><span>
        port </span><span>=</span><span> self</span><span>.</span><span>_find_next_open_port</span><span>(</span><span>minimum</span><span>)</span><span>
        </span><span>assert</span><span> port </span><span>is</span><span> </span><span>not</span><span> </span><span>None</span><span>
        </span><span>return</span><span> port
    </span>
    ```

-   Libraries or packages may define their own exceptions. When doing so they must inherit from an existing exception class. Exception names should end in `Error` and should not introduce stutter (`foo.FooError`).

-   Never use catch-all `except:` statements, or catch `Exception` or `StandardError`, unless you are

    -   re-raising the exception, or
    -   creating an isolation point in the program where exceptions are not propagated but are recorded and suppressed instead, such as protecting a thread from crashing by guarding its outermost block.

    Python is very tolerant in this regard and `except:` will really catch everything including misspelled names, sys.exit() calls, Ctrl+C interrupts, unittest failures and all kinds of other exceptions that you simply don't want to catch.

-   Minimize the amount of code in a `try`/`except` block. The larger the body of the `try`, the more likely that an exception will be raised by a line of code that you didn't expect to raise an exception. In those cases, the `try`/`except` block hides a real error.

-   Use the `finally` clause to execute code whether or not an exception is raised in the `try` block. This is often useful for cleanup, i.e., closing a file.

-   When capturing an exception, use `as` rather than a comma. For example:

    ```
    <span>try</span><span>:</span><span>
      </span><span>raise</span><span> </span><span>Error</span><span>()</span><span>
    </span><span>except</span><span> </span><span>Error</span><span> </span><span>as</span><span> error</span><span>:</span><span>
      </span><span>pass</span><span>
    </span>
    ```


### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5-Global-variables)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5-global-variables)2.5 Global variables

Avoid global variables.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_1-definition)2.5.1 Definition

Variables that are declared at the module level or as class attributes.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_2-pros)2.5.2 Pros

Occasionally useful.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_3-cons)2.5.3 Cons

Has the potential to change module behavior during the import, because assignments to global variables are done when the module is first imported.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_5_4-decision)2.5.4 Decision

Avoid global variables.

While they are technically variables, module-level constants are permitted and encouraged. For example: `MAX_HOLY_HANDGRENADE_COUNT = 3`. Constants must be named using all caps with underscores. See [Naming](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s3.16-naming) below.

If needed, globals should be declared at the module level and made internal to the module by prepending an `_` to the name. External access must be done through public module-level functions. See [Naming](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s3.16-naming) below.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6-Nested_Local_Inner-Classes-and-Functions)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6-nested_local_inner-classes-and-functions)2.6 Nested/Local/Inner Classes and Functions

Nested local functions or classes are fine when used to close over a local variable. Inner classes are fine.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_1-definition)2.6.1 Definition

A class can be defined inside of a method, function, or class. A function can be defined inside a method or function. Nested functions have read-only access to variables defined in enclosing scopes.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_2-pros)2.6.2 Pros

Allows definition of utility classes and functions that are only used inside of a very limited scope. Very [ADT](http://www.google.com/url?sa=D&q=http://en.wikipedia.org/wiki/Abstract_data_type)\-y. Commonly used for implementing decorators.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_3-cons)2.6.3 Cons

Instances of nested or local classes cannot be pickled. Nested functions and classes cannot be directly tested. Nesting can make your outer function longer and less readable.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_6_4-decision)2.6.4 Decision

They are fine with some caveats. Avoid nested functions or classes except when closing over a local value. Do not nest a function just to hide it from users of a module. Instead, prefix its name with an \_ at the module level so that it can still be accessed by tests.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7-Comprehensions-Generator-Expressions)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7-comprehensions-generator-expressions)2.7 Comprehensions & Generator Expressions

Okay to use for simple cases.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_1-definition)2.7.1 Definition

List, Dict, and Set comprehensions as well as generator expressions provide a concise and efficient way to create container types and iterators without resorting to the use of traditional loops, `map()`, `filter()`, or `lambda`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_2-pros)2.7.2 Pros

Simple comprehensions can be clearer and simpler than other dict, list, or set creation techniques. Generator expressions can be very efficient, since they avoid the creation of a list entirely.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_3-cons)2.7.3 Cons

Complicated comprehensions or generator expressions can be hard to read.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_7_4-decision)2.7.4 Decision

Okay to use for simple cases. Each portion must fit on one line: mapping expression, `for` clause, filter expression. Multiple `for` clauses or filter expressions are not permitted. Use loops instead when things get more complicated.

```
<span>Yes</span><span>:</span><span>
  result </span><span>=</span><span> </span><span>[</span><span>mapping_expr </span><span>for</span><span> value </span><span>in</span><span> iterable </span><span>if</span><span> filter_expr</span><span>]</span><span>

  result </span><span>=</span><span> </span><span>[{</span><span>'key'</span><span>:</span><span> value</span><span>}</span><span> </span><span>for</span><span> value </span><span>in</span><span> iterable
            </span><span>if</span><span> a_long_filter_expression</span><span>(</span><span>value</span><span>)]</span><span>

  result </span><span>=</span><span> </span><span>[</span><span>complicated_transform</span><span>(</span><span>x</span><span>)</span><span>
            </span><span>for</span><span> x </span><span>in</span><span> iterable </span><span>if</span><span> predicate</span><span>(</span><span>x</span><span>)]</span><span>

  descriptive_name </span><span>=</span><span> </span><span>[</span><span>
      transform</span><span>({</span><span>'key'</span><span>:</span><span> key</span><span>,</span><span> </span><span>'value'</span><span>:</span><span> value</span><span>},</span><span> color</span><span>=</span><span>'black'</span><span>)</span><span>
      </span><span>for</span><span> key</span><span>,</span><span> value </span><span>in</span><span> generate_iterable</span><span>(</span><span>some_input</span><span>)</span><span>
      </span><span>if</span><span> complicated_condition_is_met</span><span>(</span><span>key</span><span>,</span><span> value</span><span>)</span><span>
  </span><span>]</span><span>

  result </span><span>=</span><span> </span><span>[]</span><span>
  </span><span>for</span><span> x </span><span>in</span><span> range</span><span>(</span><span>10</span><span>):</span><span>
      </span><span>for</span><span> y </span><span>in</span><span> range</span><span>(</span><span>5</span><span>):</span><span>
          </span><span>if</span><span> x </span><span>*</span><span> y </span><span>&gt;</span><span> </span><span>10</span><span>:</span><span>
              result</span><span>.</span><span>append</span><span>((</span><span>x</span><span>,</span><span> y</span><span>))</span><span>

  </span><span>return</span><span> </span><span>{</span><span>x</span><span>:</span><span> complicated_transform</span><span>(</span><span>x</span><span>)</span><span>
          </span><span>for</span><span> x </span><span>in</span><span> long_generator_function</span><span>(</span><span>parameter</span><span>)</span><span>
          </span><span>if</span><span> x </span><span>is</span><span> </span><span>not</span><span> </span><span>None</span><span>}</span><span>

  squares_generator </span><span>=</span><span> </span><span>(</span><span>x</span><span>**</span><span>2</span><span> </span><span>for</span><span> x </span><span>in</span><span> range</span><span>(</span><span>10</span><span>))</span><span>

  unique_names </span><span>=</span><span> </span><span>{</span><span>user</span><span>.</span><span>name </span><span>for</span><span> user </span><span>in</span><span> users </span><span>if</span><span> user </span><span>is</span><span> </span><span>not</span><span> </span><span>None</span><span>}</span><span>

  eat</span><span>(</span><span>jelly_bean </span><span>for</span><span> jelly_bean </span><span>in</span><span> jelly_beans
      </span><span>if</span><span> jelly_bean</span><span>.</span><span>color </span><span>==</span><span> </span><span>'black'</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>
  result </span><span>=</span><span> </span><span>[</span><span>complicated_transform</span><span>(</span><span>
                x</span><span>,</span><span> some_argument</span><span>=</span><span>x</span><span>+</span><span>1</span><span>)</span><span>
            </span><span>for</span><span> x </span><span>in</span><span> iterable </span><span>if</span><span> predicate</span><span>(</span><span>x</span><span>)]</span><span>

  result </span><span>=</span><span> </span><span>[(</span><span>x</span><span>,</span><span> y</span><span>)</span><span> </span><span>for</span><span> x </span><span>in</span><span> range</span><span>(</span><span>10</span><span>)</span><span> </span><span>for</span><span> y </span><span>in</span><span> range</span><span>(</span><span>5</span><span>)</span><span> </span><span>if</span><span> x </span><span>*</span><span> y </span><span>&gt;</span><span> </span><span>10</span><span>]</span><span>

  </span><span>return</span><span> </span><span>((</span><span>x</span><span>,</span><span> y</span><span>,</span><span> z</span><span>)</span><span>
          </span><span>for</span><span> x </span><span>in</span><span> xrange</span><span>(</span><span>5</span><span>)</span><span>
          </span><span>for</span><span> y </span><span>in</span><span> xrange</span><span>(</span><span>5</span><span>)</span><span>
          </span><span>if</span><span> x </span><span>!=</span><span> y
          </span><span>for</span><span> z </span><span>in</span><span> xrange</span><span>(</span><span>5</span><span>)</span><span>
          </span><span>if</span><span> y </span><span>!=</span><span> z</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8-Default-Iterators-and-Operators)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8-default-iterators-and-operators)2.8 Default Iterators and Operators

Use default iterators and operators for types that support them, like lists, dictionaries, and files.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_1-definition)2.8.1 Definition

Container types, like dictionaries and lists, define default iterators and membership test operators (“in” and “not in”).

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_2-pros)2.8.2 Pros

The default iterators and operators are simple and efficient. They express the operation directly, without extra method calls. A function that uses default operators is generic. It can be used with any type that supports the operation.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_3-cons)2.8.3 Cons

You can't tell the type of objects by reading the method names (e.g. has\_key() means a dictionary). This is also an advantage.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_8_4-decision)2.8.4 Decision

Use default iterators and operators for types that support them, like lists, dictionaries, and files. The built-in types define iterator methods, too. Prefer these methods to methods that return lists, except that you should not mutate a container while iterating over it. Never use Python 2 specific iteration methods such as `dict.iter*()` unless necessary.

```
<span>Yes</span><span>:</span><span>  </span><span>for</span><span> key </span><span>in</span><span> adict</span><span>:</span><span> </span><span>...</span><span>
      </span><span>if</span><span> key </span><span>not</span><span> </span><span>in</span><span> adict</span><span>:</span><span> </span><span>...</span><span>
      </span><span>if</span><span> obj </span><span>in</span><span> alist</span><span>:</span><span> </span><span>...</span><span>
      </span><span>for</span><span> line </span><span>in</span><span> afile</span><span>:</span><span> </span><span>...</span><span>
      </span><span>for</span><span> k</span><span>,</span><span> v </span><span>in</span><span> adict</span><span>.</span><span>items</span><span>():</span><span> </span><span>...</span><span>
      </span><span>for</span><span> k</span><span>,</span><span> v </span><span>in</span><span> six</span><span>.</span><span>iteritems</span><span>(</span><span>adict</span><span>):</span><span> </span><span>...</span><span>
</span>
```

```
<span>No</span><span>:</span><span>   </span><span>for</span><span> key </span><span>in</span><span> adict</span><span>.</span><span>keys</span><span>():</span><span> </span><span>...</span><span>
      </span><span>if</span><span> </span><span>not</span><span> adict</span><span>.</span><span>has_key</span><span>(</span><span>key</span><span>):</span><span> </span><span>...</span><span>
      </span><span>for</span><span> line </span><span>in</span><span> afile</span><span>.</span><span>readlines</span><span>():</span><span> </span><span>...</span><span>
      </span><span>for</span><span> k</span><span>,</span><span> v </span><span>in</span><span> dict</span><span>.</span><span>iteritems</span><span>():</span><span> </span><span>...</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9-Generators)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9-generators)2.9 Generators

Use generators as needed.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9-definition)2.9 Definition

A generator function returns an iterator that yields a value each time it executes a yield statement. After it yields a value, the runtime state of the generator function is suspended until the next value is needed.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9_2-pros)2.9.2 Pros

Simpler code, because the state of local variables and control flow are preserved for each call. A generator uses less memory than a function that creates an entire list of values at once.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9_3-cons)2.9.3 Cons

None.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_9_4-decision)2.9.4 Decision

Fine. Use “Yields:” rather than “Returns:” in the docstring for generator functions.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10-Lambda-Functions)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10-lambda-functions)2.10 Lambda Functions

Okay for one-liners.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_1-definition)2.10.1 Definition

Lambdas define anonymous functions in an expression, as opposed to a statement. They are often used to define callbacks or operators for higher-order functions like `map()` and `filter()`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_2-pros)2.10.2 Pros

Convenient.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_3-cons)2.10.3 Cons

Harder to read and debug than local functions. The lack of names means stack traces are more difficult to understand. Expressiveness is limited because the function may only contain an expression.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_10_4-decision)2.10.4 Decision

Okay to use them for one-liners. If the code inside the lambda function is longer than 60-80 chars, it's probably better to define it as a regular [nested function](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#lexical-scoping).

For common operations like multiplication, use the functions from the `operator` module instead of lambda functions. For example, prefer `operator.mul` to `lambda x, y: x * y`.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11-Conditional-Expressions)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11-conditional-expressions)2.11 Conditional Expressions

Okay for simple cases.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_1-definition)2.11.1 Definition

Conditional expressions (sometimes called a “ternary operator”) are mechanisms that provide a shorter syntax for if statements. For example: `x = 1 if cond else 2`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_2-pros)2.11.2 Pros

Shorter and more convenient than an if statement.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_3-cons)2.11.3 Cons

May be harder to read than an if statement. The condition may be difficult to locate if the expression is long.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_11_4-decision)2.11.4 Decision

Okay to use for simple cases. Each portion must fit on one line: true-expression, if-expression, else-expression. Use a complete if statement when things get more complicated.

```
<span>one_line </span><span>=</span><span> </span><span>'yes'</span><span> </span><span>if</span><span> predicate</span><span>(</span><span>value</span><span>)</span><span> </span><span>else</span><span> </span><span>'no'</span><span>
slightly_split </span><span>=</span><span> </span><span>(</span><span>'yes'</span><span> </span><span>if</span><span> predicate</span><span>(</span><span>value</span><span>)</span><span>
                  </span><span>else</span><span> </span><span>'no, nein, nyet'</span><span>)</span><span>
the_longest_ternary_style_that_can_be_done </span><span>=</span><span> </span><span>(</span><span>
    </span><span>'yes, true, affirmative, confirmed, correct'</span><span>
    </span><span>if</span><span> predicate</span><span>(</span><span>value</span><span>)</span><span>
    </span><span>else</span><span> </span><span>'no, false, negative, nay'</span><span>)</span><span>
</span>
```

```
<span>bad_line_breaking </span><span>=</span><span> </span><span>(</span><span>'yes'</span><span> </span><span>if</span><span> predicate</span><span>(</span><span>value</span><span>)</span><span> </span><span>else</span><span>
                     </span><span>'no'</span><span>)</span><span>
portion_too_long </span><span>=</span><span> </span><span>(</span><span>'yes'</span><span>
                    </span><span>if</span><span> some_long_module</span><span>.</span><span>some_long_predicate_function</span><span>(</span><span>
                        really_long_variable_name</span><span>)</span><span>
                    </span><span>else</span><span> </span><span>'no, false, negative, nay'</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12-Default-Argument-Values)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12-default-argument-values)2.12 Default Argument Values

Okay in most cases.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_1-definition)2.12.1 Definition

You can specify values for variables at the end of a function's parameter list, e.g., `def foo(a, b=0):`. If `foo` is called with only one argument, `b` is set to 0. If it is called with two arguments, `b` has the value of the second argument.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_2-pros)2.12.2 Pros

Often you have a function that uses lots of default values, but on rare occasions you want to override the defaults. Default argument values provide an easy way to do this, without having to define lots of functions for the rare exceptions. As Python does not support overloaded methods/functions, default arguments are an easy way of “faking” the overloading behavior.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_3-cons)2.12.3 Cons

Default arguments are evaluated once at module load time. This may cause problems if the argument is a mutable object such as a list or a dictionary. If the function modifies the object (e.g., by appending an item to a list), the default value is modified.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_12_4-decision)2.12.4 Decision

Okay to use with the following caveat:

Do not use mutable objects as default values in the function or method definition.

```
<span>Yes</span><span>:</span><span> </span><span>def</span><span> foo</span><span>(</span><span>a</span><span>,</span><span> b</span><span>=</span><span>None</span><span>):</span><span>
         </span><span>if</span><span> b </span><span>is</span><span> </span><span>None</span><span>:</span><span>
             b </span><span>=</span><span> </span><span>[]</span><span>
</span><span>Yes</span><span>:</span><span> </span><span>def</span><span> foo</span><span>(</span><span>a</span><span>,</span><span> b</span><span>:</span><span> </span><span>Optional</span><span>[</span><span>Sequence</span><span>]</span><span> </span><span>=</span><span> </span><span>None</span><span>):</span><span>
         </span><span>if</span><span> b </span><span>is</span><span> </span><span>None</span><span>:</span><span>
             b </span><span>=</span><span> </span><span>[]</span><span>
</span><span>Yes</span><span>:</span><span> </span><span>def</span><span> foo</span><span>(</span><span>a</span><span>,</span><span> b</span><span>:</span><span> </span><span>Sequence</span><span> </span><span>=</span><span> </span><span>()):</span><span>  </span><span># Empty tuple OK since tuples are immutable</span><span>
         </span><span>...</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  </span><span>def</span><span> foo</span><span>(</span><span>a</span><span>,</span><span> b</span><span>=[]):</span><span>
         </span><span>...</span><span>
</span><span>No</span><span>:</span><span>  </span><span>def</span><span> foo</span><span>(</span><span>a</span><span>,</span><span> b</span><span>=</span><span>time</span><span>.</span><span>time</span><span>()):</span><span>  </span><span># The time the module was loaded???</span><span>
         </span><span>...</span><span>
</span><span>No</span><span>:</span><span>  </span><span>def</span><span> foo</span><span>(</span><span>a</span><span>,</span><span> b</span><span>=</span><span>FLAGS</span><span>.</span><span>my_thing</span><span>):</span><span>  </span><span># sys.argv has not yet been parsed...</span><span>
         </span><span>...</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13-Properties)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13-properties)2.13 Properties

Use properties for accessing or setting data where you would normally have used simple, lightweight accessor or setter methods.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_1-definition)2.13.1 Definition

A way to wrap method calls for getting and setting an attribute as a standard attribute access when the computation is lightweight.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_2-pros)2.13.2 Pros

Readability is increased by eliminating explicit get and set method calls for simple attribute access. Allows calculations to be lazy. Considered the Pythonic way to maintain the interface of a class. In terms of performance, allowing properties bypasses needing trivial accessor methods when a direct variable access is reasonable. This also allows accessor methods to be added in the future without breaking the interface.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_3-cons)2.13.3 Cons

Must inherit from `object` in Python 2. Can hide side-effects much like operator overloading. Can be confusing for subclasses.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_13_4-decision)2.13.4 Decision

Use properties in new code to access or set data where you would normally have used simple, lightweight accessor or setter methods. Properties should be created with the `@property` [decorator](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s2.17-function-and-method-decorators).

Inheritance with properties can be non-obvious if the property itself is not overridden. Thus one must make sure that accessor methods are called indirectly to ensure methods overridden in subclasses are called by the property (using the Template Method DP).

```
<span>Yes</span><span>:</span><span> </span><span>import</span><span> math

     </span><span>class</span><span> </span><span>Square</span><span>(</span><span>object</span><span>):</span><span>
         </span><span>"""A square with two properties: a writable area and a read-only perimeter.

         To use:
         &gt;&gt;&gt; sq = Square(3)
         &gt;&gt;&gt; sq.area
         9
         &gt;&gt;&gt; sq.perimeter
         12
         &gt;&gt;&gt; sq.area = 16
         &gt;&gt;&gt; sq.side
         4
         &gt;&gt;&gt; sq.perimeter
         16
         """</span><span>

         </span><span>def</span><span> __init__</span><span>(</span><span>self</span><span>,</span><span> side</span><span>):</span><span>
             self</span><span>.</span><span>side </span><span>=</span><span> side

         </span><span>@property</span><span>
         </span><span>def</span><span> area</span><span>(</span><span>self</span><span>):</span><span>
             </span><span>"""Area of the square."""</span><span>
             </span><span>return</span><span> self</span><span>.</span><span>_get_area</span><span>()</span><span>

         </span><span>@area</span><span>.</span><span>setter
         </span><span>def</span><span> area</span><span>(</span><span>self</span><span>,</span><span> area</span><span>):</span><span>
             </span><span>return</span><span> self</span><span>.</span><span>_set_area</span><span>(</span><span>area</span><span>)</span><span>

         </span><span>def</span><span> _get_area</span><span>(</span><span>self</span><span>):</span><span>
             </span><span>"""Indirect accessor to calculate the 'area' property."""</span><span>
             </span><span>return</span><span> self</span><span>.</span><span>side </span><span>**</span><span> </span><span>2</span><span>

         </span><span>def</span><span> _set_area</span><span>(</span><span>self</span><span>,</span><span> area</span><span>):</span><span>
             </span><span>"""Indirect setter to set the 'area' property."""</span><span>
             self</span><span>.</span><span>side </span><span>=</span><span> math</span><span>.</span><span>sqrt</span><span>(</span><span>area</span><span>)</span><span>

         </span><span>@property</span><span>
         </span><span>def</span><span> perimeter</span><span>(</span><span>self</span><span>):</span><span>
             </span><span>return</span><span> self</span><span>.</span><span>side </span><span>*</span><span> </span><span>4</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14-True_False-Evaluations)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14-true_false-evaluations)2.14 True/False Evaluations

Use the “implicit” false if at all possible.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_1-definition)2.14.1 Definition

Python evaluates certain values as `False` when in a boolean context. A quick “rule of thumb” is that all “empty” values are considered false, so `0, None, [], {}, ''` all evaluate as false in a boolean context.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_2-pros)2.14.2 Pros

Conditions using Python booleans are easier to read and less error-prone. In most cases, they're also faster.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_3-cons)2.14.3 Cons

May look strange to C/C++ developers.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_14_4-decision)2.14.4 Decision

Use the “implicit” false if possible, e.g., `if foo:` rather than `if foo != []:`. There are a few caveats that you should keep in mind though:

-   Always use `if foo is None:` (or `is not None`) to check for a `None` value-e.g., when testing whether a variable or argument that defaults to `None` was set to some other value. The other value might be a value that's false in a boolean context!

-   Never compare a boolean variable to `False` using `==`. Use `if not x:` instead. If you need to distinguish `False` from `None` then chain the expressions, such as `if not x and x is not None:`.

-   For sequences (strings, lists, tuples), use the fact that empty sequences are false, so `if seq:` and `if not seq:` are preferable to `if len(seq):` and `if not len(seq):` respectively.

-   When handling integers, implicit false may involve more risk than benefit (i.e., accidentally handling `None` as 0). You may compare a value which is known to be an integer (and is not the result of `len()`) against the integer 0.

    ```
    <span>Yes</span><span>:</span><span> </span><span>if</span><span> </span><span>not</span><span> users</span><span>:</span><span>
             </span><span>print</span><span>(</span><span>'no users'</span><span>)</span><span>

         </span><span>if</span><span> foo </span><span>==</span><span> </span><span>0</span><span>:</span><span>
             self</span><span>.</span><span>handle_zero</span><span>()</span><span>

         </span><span>if</span><span> i </span><span>%</span><span> </span><span>10</span><span> </span><span>==</span><span> </span><span>0</span><span>:</span><span>
             self</span><span>.</span><span>handle_multiple_of_ten</span><span>()</span><span>

         </span><span>def</span><span> f</span><span>(</span><span>x</span><span>=</span><span>None</span><span>):</span><span>
             </span><span>if</span><span> x </span><span>is</span><span> </span><span>None</span><span>:</span><span>
                 x </span><span>=</span><span> </span><span>[]</span><span>
    </span>
    ```

    ```
    <span>No</span><span>:</span><span>  </span><span>if</span><span> len</span><span>(</span><span>users</span><span>)</span><span> </span><span>==</span><span> </span><span>0</span><span>:</span><span>
             </span><span>print</span><span>(</span><span>'no users'</span><span>)</span><span>

         </span><span>if</span><span> foo </span><span>is</span><span> </span><span>not</span><span> </span><span>None</span><span> </span><span>and</span><span> </span><span>not</span><span> foo</span><span>:</span><span>
             self</span><span>.</span><span>handle_zero</span><span>()</span><span>

         </span><span>if</span><span> </span><span>not</span><span> i </span><span>%</span><span> </span><span>10</span><span>:</span><span>
             self</span><span>.</span><span>handle_multiple_of_ten</span><span>()</span><span>

         </span><span>def</span><span> f</span><span>(</span><span>x</span><span>=</span><span>None</span><span>):</span><span>
             x </span><span>=</span><span> x </span><span>or</span><span> </span><span>[]</span><span>
    </span>
    ```

-   Note that `'0'` (i.e., `0` as string) evaluates to true.


### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_15-Deprecated-Language-Features)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_15-deprecated-language-features)2.15 Deprecated Language Features

Use string methods instead of the `string` module where possible. Use function call syntax instead of `apply`. Use list comprehensions and `for` loops instead of `filter` and `map` when the function argument would have been an inlined lambda anyway. Use `for` loops instead of `reduce`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_15_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_15_1-definition)2.15.1 Definition

Current versions of Python provide alternative constructs that people find generally preferable.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_15_2-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_15_2-decision)2.15.2 Decision

We do not use any Python version which does not support these features, so there is no reason not to use the new styles.

```
<span>Yes</span><span>:</span><span> words </span><span>=</span><span> foo</span><span>.</span><span>split</span><span>(</span><span>':'</span><span>)</span><span>

     </span><span>[</span><span>x</span><span>[</span><span>1</span><span>]</span><span> </span><span>for</span><span> x </span><span>in</span><span> my_list </span><span>if</span><span> x</span><span>[</span><span>2</span><span>]</span><span> </span><span>==</span><span> </span><span>5</span><span>]</span><span>

     map</span><span>(</span><span>math</span><span>.</span><span>sqrt</span><span>,</span><span> data</span><span>)</span><span>    </span><span># Ok. No inlined lambda expression.</span><span>

     fn</span><span>(*</span><span>args</span><span>,</span><span> </span><span>**</span><span>kwargs</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  words </span><span>=</span><span> string</span><span>.</span><span>split</span><span>(</span><span>foo</span><span>,</span><span> </span><span>':'</span><span>)</span><span>

     map</span><span>(</span><span>lambda</span><span> x</span><span>:</span><span> x</span><span>[</span><span>1</span><span>],</span><span> filter</span><span>(</span><span>lambda</span><span> x</span><span>:</span><span> x</span><span>[</span><span>2</span><span>]</span><span> </span><span>==</span><span> </span><span>5</span><span>,</span><span> my_list</span><span>))</span><span>

     apply</span><span>(</span><span>fn</span><span>,</span><span> args</span><span>,</span><span> kwargs</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16-Lexical-Scoping)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16-lexical-scoping)2.16 Lexical Scoping

Okay to use.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_1-definition)2.16.1 Definition

A nested Python function can refer to variables defined in enclosing functions, but can not assign to them. Variable bindings are resolved using lexical scoping, that is, based on the static program text. Any assignment to a name in a block will cause Python to treat all references to that name as a local variable, even if the use precedes the assignment. If a global declaration occurs, the name is treated as a global variable.

An example of the use of this feature is:

```
<span>def</span><span> get_adder</span><span>(</span><span>summand1</span><span>):</span><span>
    </span><span>"""Returns a function that adds numbers to a given number."""</span><span>
    </span><span>def</span><span> adder</span><span>(</span><span>summand2</span><span>):</span><span>
        </span><span>return</span><span> summand1 </span><span>+</span><span> summand2

    </span><span>return</span><span> adder
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_2-pros)2.16.2 Pros

Often results in clearer, more elegant code. Especially comforting to experienced Lisp and Scheme (and Haskell and ML and ...) programmers.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_3-cons)2.16.3 Cons

Can lead to confusing bugs. Such as this example based on [PEP-0227](http://www.google.com/url?sa=D&q=http://www.python.org/dev/peps/pep-0227/):

```
<span>i </span><span>=</span><span> </span><span>4</span><span>
</span><span>def</span><span> foo</span><span>(</span><span>x</span><span>):</span><span>
    </span><span>def</span><span> bar</span><span>():</span><span>
        </span><span>print</span><span>(</span><span>i</span><span>,</span><span> end</span><span>=</span><span>''</span><span>)</span><span>
    </span><span># ...</span><span>
    </span><span># A bunch of code here</span><span>
    </span><span># ...</span><span>
    </span><span>for</span><span> i </span><span>in</span><span> x</span><span>:</span><span>  </span><span># Ah, i *is* local to foo, so this is what bar sees</span><span>
        </span><span>print</span><span>(</span><span>i</span><span>,</span><span> end</span><span>=</span><span>''</span><span>)</span><span>
    bar</span><span>()</span><span>
</span>
```

So `foo([1, 2, 3])` will print `1 2 3 3`, not `1 2 3 4`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_16_4-decision)2.16.4 Decision

Okay to use.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17-Function-and-Method-Decorators)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17-function-and-method-decorators)2.17 Function and Method Decorators

Use decorators judiciously when there is a clear advantage. Avoid `@staticmethod` and limit use of `@classmethod`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_1-definition)2.17.1 Definition

[Decorators for Functions and Methods](https://docs.python.org/3/glossary.html#term-decorator) (a.k.a “the `@` notation”). One common decorator is `@property`, used for converting ordinary methods into dynamically computed attributes. However, the decorator syntax allows for user-defined decorators as well. Specifically, for some function `my_decorator`, this:

```
<span>class</span><span> C</span><span>(</span><span>object</span><span>):</span><span>
    </span><span>@my_decorator</span><span>
    </span><span>def</span><span> method</span><span>(</span><span>self</span><span>):</span><span>
        </span><span># method body ...</span><span>
</span>
```

is equivalent to:

```
<span>class</span><span> C</span><span>(</span><span>object</span><span>):</span><span>
    </span><span>def</span><span> method</span><span>(</span><span>self</span><span>):</span><span>
        </span><span># method body ...</span><span>
    method </span><span>=</span><span> my_decorator</span><span>(</span><span>method</span><span>)</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_2-pros)2.17.2 Pros

Elegantly specifies some transformation on a method; the transformation might eliminate some repetitive code, enforce invariants, etc.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_3-cons)2.17.3 Cons

Decorators can perform arbitrary operations on a function's arguments or return values, resulting in surprising implicit behavior. Additionally, decorators execute at import time. Failures in decorator code are pretty much impossible to recover from.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_17_4-decision)2.17.4 Decision

Use decorators judiciously when there is a clear advantage. Decorators should follow the same import and naming guidelines as functions. Decorator pydoc should clearly state that the function is a decorator. Write unit tests for decorators.

Avoid external dependencies in the decorator itself (e.g. don't rely on files, sockets, database connections, etc.), since they might not be available when the decorator runs (at import time, perhaps from `pydoc` or other tools). A decorator that is called with valid parameters should (as much as possible) be guaranteed to succeed in all cases.

Decorators are a special case of “top level code” - see [main](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s3.17-main) for more discussion.

Never use `@staticmethod` unless forced to in order to integrate with an API defined in an existing library. Write a module level function instead.

Use `@classmethod` only when writing a named constructor or a class-specific routine that modifies necessary global state such as a process-wide cache.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_18-Threading)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_18-threading)2.18 Threading

Do not rely on the atomicity of built-in types.

While Python‘s built-in data types such as dictionaries appear to have atomic operations, there are corner cases where they aren’t atomic (e.g. if `__hash__` or `__eq__` are implemented as Python methods) and their atomicity should not be relied upon. Neither should you rely on atomic variable assignment (since this in turn depends on dictionaries).

Use the Queue module's `Queue` data type as the preferred way to communicate data between threads. Otherwise, use the threading module and its locking primitives. Learn about the proper use of condition variables so you can use `threading.Condition` instead of using lower-level locks.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19-Power-Features)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19-power-features)2.19 Power Features

Avoid these features.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_1-definition)2.19.1 Definition

Python is an extremely flexible language and gives you many fancy features such as custom metaclasses, access to bytecode, on-the-fly compilation, dynamic inheritance, object reparenting, import hacks, reflection (e.g. some uses of `getattr()`), modification of system internals, etc.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_2-pros)2.19.2 Pros

These are powerful language features. They can make your code more compact.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_3-cons)2.19.3 Cons

It‘s very tempting to use these “cool” features when they’re not absolutely necessary. It‘s harder to read, understand, and debug code that’s using unusual features underneath. It doesn't seem that way at first (to the original author), but when revisiting the code, it tends to be more difficult than code that is longer but is straightforward.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_19_4-decision)2.19.4 Decision

Avoid these features in your code.

Standard library modules and classes that internally use these features are okay to use (for example, `abc.ABCMeta`, `collections.namedtuple`, `dataclasses`, and `enum`).

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20-Modern-Python_Python-3-and-from-future_imports)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20-modern-python_python-3-and-from-future_imports)2.20 Modern Python: Python 3 and from \_\_future\_\_ imports

Python 3 is here! While not every project is ready to use it yet, all code should be written to be 3 compatible (and tested under 3 when possible).

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_1-definition)2.20.1 Definition

Python 3 is a significant change in the Python language. While existing code is often written with 2.7 in mind, there are some simple things to do to make code more explicit about its intentions and thus better prepared for use under Python 3 without modification.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_2-pros)2.20.2 Pros

Code written with Python 3 in mind is more explicit and easier to get running under Python 3 once all of the dependencies of your project are ready.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_3-cons)2.20.3 Cons

Some people find the additional boilerplate to be ugly. It‘s unusual to add imports to a module that doesn’t actually require the features added by the import.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_20_4-decision)2.20.4 Decision

##### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#from-future_imports)from \_\_future\_\_ imports

Use of `from __future__ import` statements is encouraged. All new code should contain the following and existing code should be updated to be compatible when possible:

```
<span>from</span><span> __future__ </span><span>import</span><span> absolute_import
</span><span>from</span><span> __future__ </span><span>import</span><span> division
</span><span>from</span><span> __future__ </span><span>import</span><span> print_function
</span>
```

If you are not already familiar with those, read up on each here: [absolute imports](https://www.python.org/dev/peps/pep-0328/), [new `/` division behavior](https://www.python.org/dev/peps/pep-0238/), and [the print function](https://www.python.org/dev/peps/pep-3105/).

Please don‘t omit or remove these imports, even if they’re not currently used in the module, unless the code is Python 3 only. It is better to always have the future imports in all files so that they are not forgotten during later edits when someone starts using such a feature.

There are other `from __future__` import statements. Use them as you see fit. We do not include `unicode_literals` in our recommendations as it is not a clear win due to implicit default codec conversion consequences it introduces in many places within Python 2.7. Most code is better off with explicit use of `b''` and `u''` bytes and unicode string literals as necessary.

##### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#The-six_future_or-past-libraries)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#the-six_future_or-past-libraries)The six, future, or past libraries

When your project needs to actively support use under both Python 2 and 3, use the [six](https://pypi.org/project/six/), [future](https://pypi.org/project/future/), and [past](https://pypi.org/project/past/) libraries as you see fit. They exist to make your code cleaner and life easier.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21-Type-Annotated-Code)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21-type-annotated-code)2.21 Type Annotated Code

You can annotate Python 3 code with type hints according to [PEP-484](https://www.python.org/dev/peps/pep-0484/), and type-check the code at build time with a type checking tool like [pytype](https://github.com/google/pytype).

Type annotations can be in the source or in a [stub pyi file](https://www.python.org/dev/peps/pep-0484/#stub-files). Whenever possible, annotations should be in the source. Use pyi files for third-party or extension modules.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_1-Definition)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_1-definition)2.21.1 Definition

Type annotations (or “type hints”) are for function or method arguments and return values:

```
<span>def</span><span> func</span><span>(</span><span>a</span><span>:</span><span> int</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>List</span><span>[</span><span>int</span><span>]:</span><span>
</span>
```

You can also declare the type of a variable using a special comment:

```
<span>a </span><span>=</span><span> </span><span>SomeFunc</span><span>()</span><span>  </span><span># type: SomeType</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_2-Pros)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_2-pros)2.21.2 Pros

Type annotations improve the readability and maintainability of your code. The type checker will convert many runtime errors to build-time errors, and reduce your ability to use [Power Features](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#power-features).

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_3-Cons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_3-cons)2.21.3 Cons

You will have to keep the type declarations up to date. You might see type errors that you think are valid code. Use of a [type checker](https://github.com/google/pytype) may reduce your ability to use [Power Features](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#power-features).

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_4-Decision)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#2_21_4-decision)2.21.4 Decision

You are strongly encouraged to enable Python type analysis when updating code. When adding or modifying public APIs, include type annotations and enable checking via pytype in the build system. As static analysis is relatively new to Python, we acknowledge that undesired side-effects (such as wrongly inferred types) may prevent adoption by some projects. In those situations, authors are encouraged to add a comment with a TODO or link to a bug describing the issue(s) currently preventing type annotation adoption in the BUILD file or in the code itself as appropriate.

## [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3-Python-Style-Rules)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3-python-style-rules)3 Python Style Rules

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_1-Semicolons)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_1-semicolons)3.1 Semicolons

Do not terminate your lines with semicolons, and do not use semicolons to put two statements on the same line.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_2-Line-length)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_2-line-length)3.2 Line length

Maximum line length is _80 characters_.

Explicit exceptions to the 80 character limit:

-   Long import statements.
-   URLs, pathnames, or long flags in comments.
-   Long string module level constants not containing whitespace that would be inconvenient to split across lines such as URLs or pathnames.
-   Pylint disable comments. (e.g.: `# pylint: disable=invalid-name`)

Do not use backslash line continuation except for `with` statements requiring three or more context managers.

Make use of Python's [implicit line joining inside parentheses, brackets and braces](http://docs.python.org/reference/lexical_analysis.html#implicit-line-joining). If necessary, you can add an extra pair of parentheses around an expression.

```
<span>Yes</span><span>:</span><span> foo_bar</span><span>(</span><span>self</span><span>,</span><span> width</span><span>,</span><span> height</span><span>,</span><span> color</span><span>=</span><span>'black'</span><span>,</span><span> design</span><span>=</span><span>None</span><span>,</span><span> x</span><span>=</span><span>'foo'</span><span>,</span><span>
             emphasis</span><span>=</span><span>None</span><span>,</span><span> highlight</span><span>=</span><span>0</span><span>)</span><span>

     </span><span>if</span><span> </span><span>(</span><span>width </span><span>==</span><span> </span><span>0</span><span> </span><span>and</span><span> height </span><span>==</span><span> </span><span>0</span><span> </span><span>and</span><span>
         color </span><span>==</span><span> </span><span>'red'</span><span> </span><span>and</span><span> emphasis </span><span>==</span><span> </span><span>'strong'</span><span>):</span><span>
</span>
```

When a literal string won't fit on a single line, use parentheses for implicit line joining.

```
<span>x </span><span>=</span><span> </span><span>(</span><span>'This will build a very long long '</span><span>
     </span><span>'long long long long long long string'</span><span>)</span><span>
</span>
```

Within comments, put long URLs on their own line if necessary.

```
<span>Yes</span><span>:</span><span>  </span><span># See details at</span><span>
      </span><span># http://www.example.com/us/developer/documentation/api/content/v2.0/csv_file_name_extension_full_specification.html</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  </span><span># See details at</span><span>
     </span><span># http://www.example.com/us/developer/documentation/api/content/\</span><span>
     </span><span># v2.0/csv_file_name_extension_full_specification.html</span><span>
</span>
```

It is permissible to use backslash continuation when defining a `with` statement whose expressions span three or more lines. For two lines of expressions, use a nested `with` statement:

```
<span>Yes</span><span>:</span><span>  </span><span>with</span><span> very_long_first_expression_function</span><span>()</span><span> </span><span>as</span><span> spam</span><span>,</span><span> \
           very_long_second_expression_function</span><span>()</span><span> </span><span>as</span><span> beans</span><span>,</span><span> \
           third_thing</span><span>()</span><span> </span><span>as</span><span> eggs</span><span>:</span><span>
          place_order</span><span>(</span><span>eggs</span><span>,</span><span> beans</span><span>,</span><span> spam</span><span>,</span><span> beans</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  </span><span>with</span><span> </span><span>VeryLongFirstExpressionFunction</span><span>()</span><span> </span><span>as</span><span> spam</span><span>,</span><span> \
          </span><span>VeryLongSecondExpressionFunction</span><span>()</span><span> </span><span>as</span><span> beans</span><span>:</span><span>
       </span><span>PlaceOrder</span><span>(</span><span>eggs</span><span>,</span><span> beans</span><span>,</span><span> spam</span><span>,</span><span> beans</span><span>)</span><span>
</span>
```

```
<span>Yes</span><span>:</span><span>  </span><span>with</span><span> very_long_first_expression_function</span><span>()</span><span> </span><span>as</span><span> spam</span><span>:</span><span>
          </span><span>with</span><span> very_long_second_expression_function</span><span>()</span><span> </span><span>as</span><span> beans</span><span>:</span><span>
              place_order</span><span>(</span><span>beans</span><span>,</span><span> spam</span><span>)</span><span>
</span>
```

Make note of the indentation of the elements in the line continuation examples above; see the [indentation](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s3.4-indentation) section for explanation.

In all other cases where a line exceeds 80 characters, and the [yapf](https://github.com/google/yapf/) auto-formatter does not help bring the line below the limit, the line is allowed to exceed this maximum.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_3-Parentheses)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_3-parentheses)3.3 Parentheses

Use parentheses sparingly.

It is fine, though not required, to use parentheses around tuples. Do not use them in return statements or conditional statements unless using parentheses for implied line continuation or to indicate a tuple.

```
<span>Yes</span><span>:</span><span> </span><span>if</span><span> foo</span><span>:</span><span>
         bar</span><span>()</span><span>
     </span><span>while</span><span> x</span><span>:</span><span>
         x </span><span>=</span><span> bar</span><span>()</span><span>
     </span><span>if</span><span> x </span><span>and</span><span> y</span><span>:</span><span>
         bar</span><span>()</span><span>
     </span><span>if</span><span> </span><span>not</span><span> x</span><span>:</span><span>
         bar</span><span>()</span><span>
     </span><span># For a 1 item tuple the ()s are more visually obvious than the comma.</span><span>
     onesie </span><span>=</span><span> </span><span>(</span><span>foo</span><span>,)</span><span>
     </span><span>return</span><span> foo
     </span><span>return</span><span> spam</span><span>,</span><span> beans
     </span><span>return</span><span> </span><span>(</span><span>spam</span><span>,</span><span> beans</span><span>)</span><span>
     </span><span>for</span><span> </span><span>(</span><span>x</span><span>,</span><span> y</span><span>)</span><span> </span><span>in</span><span> dict</span><span>.</span><span>items</span><span>():</span><span> </span><span>...</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  </span><span>if</span><span> </span><span>(</span><span>x</span><span>):</span><span>
         bar</span><span>()</span><span>
     </span><span>if</span><span> </span><span>not</span><span>(</span><span>x</span><span>):</span><span>
         bar</span><span>()</span><span>
     </span><span>return</span><span> </span><span>(</span><span>foo</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_4-Indentation)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_4-indentation)3.4 Indentation

Indent your code blocks with _4 spaces_.

Never use tabs or mix tabs and spaces. In cases of implied line continuation, you should align wrapped elements either vertically, as per the examples in the [line length](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s3.2-line-length) section; or using a hanging indent of 4 spaces, in which case there should be nothing after the open parenthesis or bracket on the first line.

```
<span>Yes</span><span>:</span><span>   </span><span># Aligned with opening delimiter</span><span>
       foo </span><span>=</span><span> long_function_name</span><span>(</span><span>var_one</span><span>,</span><span> var_two</span><span>,</span><span>
                                var_three</span><span>,</span><span> var_four</span><span>)</span><span>
       meal </span><span>=</span><span> </span><span>(</span><span>spam</span><span>,</span><span>
               beans</span><span>)</span><span>

       </span><span># Aligned with opening delimiter in a dictionary</span><span>
       foo </span><span>=</span><span> </span><span>{</span><span>
           long_dictionary_key</span><span>:</span><span> value1 </span><span>+</span><span>
                                value2</span><span>,</span><span>
           </span><span>...</span><span>
       </span><span>}</span><span>

       </span><span># 4-space hanging indent; nothing on first line</span><span>
       foo </span><span>=</span><span> long_function_name</span><span>(</span><span>
           var_one</span><span>,</span><span> var_two</span><span>,</span><span> var_three</span><span>,</span><span>
           var_four</span><span>)</span><span>
       meal </span><span>=</span><span> </span><span>(</span><span>
           spam</span><span>,</span><span>
           beans</span><span>)</span><span>

       </span><span># 4-space hanging indent in a dictionary</span><span>
       foo </span><span>=</span><span> </span><span>{</span><span>
           long_dictionary_key</span><span>:</span><span>
               long_dictionary_value</span><span>,</span><span>
           </span><span>...</span><span>
       </span><span>}</span><span>
</span>
```

```
<span>No</span><span>:</span><span>    </span><span># Stuff on first line forbidden</span><span>
       foo </span><span>=</span><span> long_function_name</span><span>(</span><span>var_one</span><span>,</span><span> var_two</span><span>,</span><span>
           var_three</span><span>,</span><span> var_four</span><span>)</span><span>
       meal </span><span>=</span><span> </span><span>(</span><span>spam</span><span>,</span><span>
           beans</span><span>)</span><span>

       </span><span># 2-space hanging indent forbidden</span><span>
       foo </span><span>=</span><span> long_function_name</span><span>(</span><span>
         var_one</span><span>,</span><span> var_two</span><span>,</span><span> var_three</span><span>,</span><span>
         var_four</span><span>)</span><span>

       </span><span># No hanging indent in a dictionary</span><span>
       foo </span><span>=</span><span> </span><span>{</span><span>
           long_dictionary_key</span><span>:</span><span>
           long_dictionary_value</span><span>,</span><span>
           </span><span>...</span><span>
       </span><span>}</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_4_1-Trailing-commas-in-sequences-of-items)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_4_1-trailing-commas-in-sequences-of-items)3.4.1 Trailing commas in sequences of items?

Trailing commas in sequences of items are recommended only when the closing container token `]`, `)`, or `}` does not appear on the same line as the final element. The presence of a trailing comma is also used as a hint to our Python code auto-formatter [YAPF](https://pypi.org/project/yapf/) to direct it to auto-format the container of items to one item per line when the `,` after the final element is present.

```
<span>Yes</span><span>:</span><span>   golomb3 </span><span>=</span><span> </span><span>[</span><span>0</span><span>,</span><span> </span><span>1</span><span>,</span><span> </span><span>3</span><span>]</span><span>
</span><span>Yes</span><span>:</span><span>   golomb4 </span><span>=</span><span> </span><span>[</span><span>
           </span><span>0</span><span>,</span><span>
           </span><span>1</span><span>,</span><span>
           </span><span>4</span><span>,</span><span>
           </span><span>6</span><span>,</span><span>
       </span><span>]</span><span>
</span>
```

```
<span>No</span><span>:</span><span>    golomb4 </span><span>=</span><span> </span><span>[</span><span>
           </span><span>0</span><span>,</span><span>
           </span><span>1</span><span>,</span><span>
           </span><span>4</span><span>,</span><span>
           </span><span>6</span><span>
       </span><span>]</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_5-Blank-Lines)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_5-blank-lines)3.5 Blank Lines

Two blank lines between top-level definitions, be they function or class definitions. One blank line between method definitions and between the `class` line and the first method. No blank line following a `def` line. Use single blank lines as you judge appropriate within functions or methods.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_6-Whitespace)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_6-whitespace)3.6 Whitespace

Follow standard typographic rules for the use of spaces around punctuation.

No whitespace inside parentheses, brackets or braces.

```
<span>Yes</span><span>:</span><span> spam</span><span>(</span><span>ham</span><span>[</span><span>1</span><span>],</span><span> </span><span>{</span><span>eggs</span><span>:</span><span> </span><span>2</span><span>},</span><span> </span><span>[])</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  spam</span><span>(</span><span> ham</span><span>[</span><span> </span><span>1</span><span> </span><span>],</span><span> </span><span>{</span><span> eggs</span><span>:</span><span> </span><span>2</span><span> </span><span>},</span><span> </span><span>[</span><span> </span><span>]</span><span> </span><span>)</span><span>
</span>
```

No whitespace before a comma, semicolon, or colon. Do use whitespace after a comma, semicolon, or colon, except at the end of the line.

```
<span>Yes</span><span>:</span><span> </span><span>if</span><span> x </span><span>==</span><span> </span><span>4</span><span>:</span><span>
         </span><span>print</span><span>(</span><span>x</span><span>,</span><span> y</span><span>)</span><span>
     x</span><span>,</span><span> y </span><span>=</span><span> y</span><span>,</span><span> x
</span>
```

```
<span>No</span><span>:</span><span>  </span><span>if</span><span> x </span><span>==</span><span> </span><span>4</span><span> </span><span>:</span><span>
         </span><span>print</span><span>(</span><span>x </span><span>,</span><span> y</span><span>)</span><span>
     x </span><span>,</span><span> y </span><span>=</span><span> y </span><span>,</span><span> x
</span>
```

No whitespace before the open paren/bracket that starts an argument list, indexing or slicing.

```
<span>Yes</span><span>:</span><span> spam</span><span>(</span><span>1</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  spam </span><span>(</span><span>1</span><span>)</span><span>
</span>
```

```
<span>Yes</span><span>:</span><span> dict</span><span>[</span><span>'key'</span><span>]</span><span> </span><span>=</span><span> list</span><span>[</span><span>index</span><span>]</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  dict </span><span>[</span><span>'key'</span><span>]</span><span> </span><span>=</span><span> list </span><span>[</span><span>index</span><span>]</span><span>
</span>
```

No trailing whitespace.

Surround binary operators with a single space on either side for assignment (`=`), comparisons (`==, <, >, !=, <>, <=, >=, in, not in, is, is not`), and Booleans (`and, or, not`). Use your better judgment for the insertion of spaces around arithmetic operators (`+`, `-`, `*`, `/`, `//`, `%`, `**`, `@`).

```
<span>Yes</span><span>:</span><span> x </span><span>==</span><span> </span><span>1</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  x</span><span>&lt;</span><span>1</span><span>
</span>
```

Never use spaces around `=` when passing keyword arguments or defining a default parameter value, with one exception: [when a type annotation is present](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-default-values), _do_ use spaces around the `=` for the default parameter value.

```
<span>Yes</span><span>:</span><span> </span><span>def</span><span> complex</span><span>(</span><span>real</span><span>,</span><span> imag</span><span>=</span><span>0.0</span><span>):</span><span> </span><span>return</span><span> </span><span>Magic</span><span>(</span><span>r</span><span>=</span><span>real</span><span>,</span><span> i</span><span>=</span><span>imag</span><span>)</span><span>
</span><span>Yes</span><span>:</span><span> </span><span>def</span><span> complex</span><span>(</span><span>real</span><span>,</span><span> imag</span><span>:</span><span> float </span><span>=</span><span> </span><span>0.0</span><span>):</span><span> </span><span>return</span><span> </span><span>Magic</span><span>(</span><span>r</span><span>=</span><span>real</span><span>,</span><span> i</span><span>=</span><span>imag</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>  </span><span>def</span><span> complex</span><span>(</span><span>real</span><span>,</span><span> imag </span><span>=</span><span> </span><span>0.0</span><span>):</span><span> </span><span>return</span><span> </span><span>Magic</span><span>(</span><span>r </span><span>=</span><span> real</span><span>,</span><span> i </span><span>=</span><span> imag</span><span>)</span><span>
</span><span>No</span><span>:</span><span>  </span><span>def</span><span> complex</span><span>(</span><span>real</span><span>,</span><span> imag</span><span>:</span><span> float</span><span>=</span><span>0.0</span><span>):</span><span> </span><span>return</span><span> </span><span>Magic</span><span>(</span><span>r </span><span>=</span><span> real</span><span>,</span><span> i </span><span>=</span><span> imag</span><span>)</span><span>
</span>
```

Don't use spaces to vertically align tokens on consecutive lines, since it becomes a maintenance burden (applies to `:`, `#`, `=`, etc.):

```
<span>Yes</span><span>:</span><span>
  foo </span><span>=</span><span> </span><span>1000</span><span>  </span><span># comment</span><span>
  long_name </span><span>=</span><span> </span><span>2</span><span>  </span><span># comment that should not be aligned</span><span>

  dictionary </span><span>=</span><span> </span><span>{</span><span>
      </span><span>'foo'</span><span>:</span><span> </span><span>1</span><span>,</span><span>
      </span><span>'long_name'</span><span>:</span><span> </span><span>2</span><span>,</span><span>
  </span><span>}</span><span>
</span>
```

```
<span>No</span><span>:</span><span>
  foo       </span><span>=</span><span> </span><span>1000</span><span>  </span><span># comment</span><span>
  long_name </span><span>=</span><span> </span><span>2</span><span>     </span><span># comment that should not be aligned</span><span>

  dictionary </span><span>=</span><span> </span><span>{</span><span>
      </span><span>'foo'</span><span>      </span><span>:</span><span> </span><span>1</span><span>,</span><span>
      </span><span>'long_name'</span><span>:</span><span> </span><span>2</span><span>,</span><span>
  </span><span>}</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_7-Shebang-Line)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_7-shebang-line)3.7 Shebang Line

Most `.py` files do not need to start with a `#!` line. Start the main file of a program with `#!/usr/bin/python` with an optional single digit `2` or `3` suffix per [PEP-394](https://www.google.com/url?sa=D&q=http://www.python.org/dev/peps/pep-0394/).

This line is used by the kernel to find the Python interpreter, but is ignored by Python when importing modules. It is only necessary on a file that will be executed directly.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8-Comments-and-Docstrings)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8-comments-and-docstrings)3.8 Comments and Docstrings

Be sure to use the right style for module, function, method docstrings and inline comments.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_1-Docstrings)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_1-docstrings)3.8.1 Docstrings

Python uses _docstrings_ to document code. A docstring is a string that is the first statement in a package, module, class or function. These strings can be extracted automatically through the `__doc__` member of the object and are used by `pydoc`. (Try running `pydoc` on your module to see how it looks.) Always use the three double-quote `"""` format for docstrings (per [PEP 257](https://www.google.com/url?sa=D&q=http://www.python.org/dev/peps/pep-0257/)). A docstring should be organized as a summary line (one physical line) terminated by a period, question mark, or exclamation point, followed by a blank line, followed by the rest of the docstring starting at the same cursor position as the first quote of the first line. There are more formatting guidelines for docstrings below.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_2-Modules)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_2-modules)3.8.2 Modules

Every file should contain license boilerplate. Choose the appropriate boilerplate for the license used by the project (for example, Apache 2.0, BSD, LGPL, GPL)

Files should start with a docstring describing the contents and usage of the module.

```
<span>"""A one line summary of the module or program, terminated by a period.

Leave one blank line.  The rest of this docstring should contain an
overall description of the module or program.  Optionally, it may also
contain a brief description of exported classes and functions and/or usage
examples.

  Typical usage example:

  foo = ClassFoo()
  bar = foo.FunctionBar()
"""</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_3-Functions-and-Methods)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_3-functions-and-methods)3.8.3 Functions and Methods

In this section, “function” means a method, function, or generator.

A function must have a docstring, unless it meets all of the following criteria:

-   not externally visible
-   very short
-   obvious

A docstring should give enough information to write a call to the function without reading the function‘s code. The docstring should be descriptive-style (`"""Fetches rows from a Bigtable."""`) rather than imperative-style (`"""Fetch rows from a Bigtable."""`), except for `@property` data descriptors, which should use the same style as attributes. A docstring should describe the function’s calling syntax and its semantics, not its implementation. For tricky code, comments alongside the code are more appropriate than using docstrings.

A method that overrides a method from a base class may have a simple docstring sending the reader to its overridden method‘s docstring, such as `"""See base class."""`. The rationale is that there is no need to repeat in many places documentation that is already present in the base method’s docstring. However, if the overriding method's behavior is substantially different from the overridden method, or details need to be provided (e.g., documenting additional side effects), a docstring with at least those differences is required on the overriding method.

Certain aspects of a function should be documented in special sections, listed below. Each section begins with a heading line, which ends with a colon. All sections other than the heading should maintain a hanging indent of two or four spaces (be consistent within a file). These sections can be omitted in cases where the function's name and signature are informative enough that it can be aptly described using a one-line docstring.

[_Args:_](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#doc-function-args) : List each parameter by name. A description should follow the name, and be separated by a colon and a space. If the description is too long to fit on a single 80-character line, use a hanging indent of 2 or 4 spaces (be consistent with the rest of the file).

```
The description should include required type(s) if the code does not contain
a corresponding type annotation. If a function accepts `*foo` (variable
length argument lists) and/or `**bar` (arbitrary keyword arguments), they
should be listed as `*foo` and `**bar`.
```

[_Returns:_ (or _Yields:_ for generators)](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#doc-function-returns) : Describe the type and semantics of the return value. If the function only returns None, this section is not required. It may also be omitted if the docstring starts with Returns or Yields (e.g. `"""Returns row from Bigtable as a tuple of strings."""`) and the opening sentence is sufficient to describe return value.

[_Raises:_](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#doc-function-raises) : List all exceptions that are relevant to the interface. You should not document exceptions that get raised if the API specified in the docstring is violated (because this would paradoxically make behavior under violation of the API part of the API).

```
<span>def</span><span> fetch_bigtable_rows</span><span>(</span><span>big_table</span><span>,</span><span> keys</span><span>,</span><span> other_silly_variable</span><span>=</span><span>None</span><span>):</span><span>
    </span><span>"""Fetches rows from a Bigtable.

    Retrieves rows pertaining to the given keys from the Table instance
    represented by big_table.  Silly things may happen if
    other_silly_variable is not None.

    Args:
        big_table: An open Bigtable Table instance.
        keys: A sequence of strings representing the key of each table row
            to fetch.
        other_silly_variable: Another optional variable, that has a much
            longer name than the other args, and which does nothing.

    Returns:
        A dict mapping keys to the corresponding table row data
        fetched. Each row is represented as a tuple of strings. For
        example:

        {'Serak': ('Rigel VII', 'Preparer'),
         'Zim': ('Irk', 'Invader'),
         'Lrrr': ('Omicron Persei 8', 'Emperor')}

        If a key from the keys argument is missing from the dictionary,
        then that row was not found in the table.

    Raises:
        IOError: An error occurred accessing the bigtable.Table object.
    """</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_4-Classes)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_4-classes)3.8.4 Classes

Classes should have a docstring below the class definition describing the class. If your class has public attributes, they should be documented here in an `Attributes` section and follow the same formatting as a [function's `Args`](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#doc-function-args) section.

```
<span>class</span><span> </span><span>SampleClass</span><span>(</span><span>object</span><span>):</span><span>
    </span><span>"""Summary of class here.

    Longer class information....
    Longer class information....

    Attributes:
        likes_spam: A boolean indicating if we like SPAM or not.
        eggs: An integer count of the eggs we have laid.
    """</span><span>

    </span><span>def</span><span> __init__</span><span>(</span><span>self</span><span>,</span><span> likes_spam</span><span>=</span><span>False</span><span>):</span><span>
        </span><span>"""Inits SampleClass with blah."""</span><span>
        self</span><span>.</span><span>likes_spam </span><span>=</span><span> likes_spam
        self</span><span>.</span><span>eggs </span><span>=</span><span> </span><span>0</span><span>

    </span><span>def</span><span> public_method</span><span>(</span><span>self</span><span>):</span><span>
        </span><span>"""Performs operation blah."""</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_5-Block-and-Inline-Comments)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_5-block-and-inline-comments)3.8.5 Block and Inline Comments

The final place to have comments is in tricky parts of the code. If you're going to have to explain it at the next [code review](http://en.wikipedia.org/wiki/Code_review), you should comment it now. Complicated operations get a few lines of comments before the operations commence. Non-obvious ones get comments at the end of the line.

```
<span># We use a weighted dictionary search to find out where i is in</span><span>
</span><span># the array.  We extrapolate position based on the largest num</span><span>
</span><span># in the array and the array size and then do binary search to</span><span>
</span><span># get the exact number.</span><span>

</span><span>if</span><span> i </span><span>&amp;</span><span> </span><span>(</span><span>i</span><span>-</span><span>1</span><span>)</span><span> </span><span>==</span><span> </span><span>0</span><span>:</span><span>  </span><span># True if i is 0 or a power of 2.</span><span>
</span>
```

To improve legibility, these comments should start at least 2 spaces away from the code with the comment character `#`, followed by at least one space before the text of the comment itself.

On the other hand, never describe the code. Assume the person reading the code knows Python (though not what you're trying to do) better than you do.

```
<span># BAD COMMENT: Now go through the b array and make sure whenever i occurs</span><span>
</span><span># the next element is i+1</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_6-Punctuation_Spelling-and-Grammar)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_8_6-punctuation_spelling-and-grammar)3.8.6 Punctuation, Spelling and Grammar

Pay attention to punctuation, spelling, and grammar; it is easier to read well-written comments than badly written ones.

Comments should be as readable as narrative text, with proper capitalization and punctuation. In many cases, complete sentences are more readable than sentence fragments. Shorter comments, such as comments at the end of a line of code, can sometimes be less formal, but you should be consistent with your style.

Although it can be frustrating to have a code reviewer point out that you are using a comma when you should be using a semicolon, it is very important that source code maintain a high level of clarity and readability. Proper punctuation, spelling, and grammar help with that goal.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_9-Classes)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_9-classes)3.9 Classes

If a class inherits from no other base classes, explicitly inherit from `object`. This also applies to nested classes.

```
<span>Yes</span><span>:</span><span> </span><span>class</span><span> </span><span>SampleClass</span><span>(</span><span>object</span><span>):</span><span>
         </span><span>pass</span><span>


     </span><span>class</span><span> </span><span>OuterClass</span><span>(</span><span>object</span><span>):</span><span>

         </span><span>class</span><span> </span><span>InnerClass</span><span>(</span><span>object</span><span>):</span><span>
             </span><span>pass</span><span>


     </span><span>class</span><span> </span><span>ChildClass</span><span>(</span><span>ParentClass</span><span>):</span><span>
         </span><span>"""Explicitly inherits from another class already."""</span><span>

</span>
```

```
<span>No</span><span>:</span><span> </span><span>class</span><span> </span><span>SampleClass</span><span>:</span><span>
        </span><span>pass</span><span>


    </span><span>class</span><span> </span><span>OuterClass</span><span>:</span><span>

        </span><span>class</span><span> </span><span>InnerClass</span><span>:</span><span>
            </span><span>pass</span><span>
</span>
```

Inheriting from `object` is needed to make properties work properly in Python 2 and can protect your code from potential incompatibility with Python 3. It also defines special methods that implement the default semantics of objects including `__new__`, `__init__`, `__delattr__`, `__getattribute__`, `__setattr__`, `__hash__`, `__repr__`, and `__str__`.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_10-Strings)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_10-strings)3.10 Strings

Use the `format` method or the `%` operator for formatting strings, even when the parameters are all strings. Use your best judgment to decide between `+` and `%` (or `format`) though.

```
<span>Yes</span><span>:</span><span> x </span><span>=</span><span> a </span><span>+</span><span> b
     x </span><span>=</span><span> </span><span>'%s, %s!'</span><span> </span><span>%</span><span> </span><span>(</span><span>imperative</span><span>,</span><span> expletive</span><span>)</span><span>
     x </span><span>=</span><span> </span><span>'{}, {}'</span><span>.</span><span>format</span><span>(</span><span>first</span><span>,</span><span> second</span><span>)</span><span>
     x </span><span>=</span><span> </span><span>'name: %s; score: %d'</span><span> </span><span>%</span><span> </span><span>(</span><span>name</span><span>,</span><span> n</span><span>)</span><span>
     x </span><span>=</span><span> </span><span>'name: {}; score: {}'</span><span>.</span><span>format</span><span>(</span><span>name</span><span>,</span><span> n</span><span>)</span><span>
     x </span><span>=</span><span> f</span><span>'name: {name}; score: {n}'</span><span>  </span><span># Python 3.6+</span><span>
</span>
```

```
<span>No</span><span>:</span><span> x </span><span>=</span><span> </span><span>'%s%s'</span><span> </span><span>%</span><span> </span><span>(</span><span>a</span><span>,</span><span> b</span><span>)</span><span>  </span><span># use + in this case</span><span>
    x </span><span>=</span><span> </span><span>'{}{}'</span><span>.</span><span>format</span><span>(</span><span>a</span><span>,</span><span> b</span><span>)</span><span>  </span><span># use + in this case</span><span>
    x </span><span>=</span><span> first </span><span>+</span><span> </span><span>', '</span><span> </span><span>+</span><span> second
    x </span><span>=</span><span> </span><span>'name: '</span><span> </span><span>+</span><span> name </span><span>+</span><span> </span><span>'; score: '</span><span> </span><span>+</span><span> str</span><span>(</span><span>n</span><span>)</span><span>
</span>
```

Avoid using the `+` and `+=` operators to accumulate a string within a loop. Since strings are immutable, this creates unnecessary temporary objects and results in quadratic rather than linear running time. Instead, add each substring to a list and `''.join` the list after the loop terminates (or, write each substring to a `io.BytesIO` buffer).

```
<span>Yes</span><span>:</span><span> items </span><span>=</span><span> </span><span>[</span><span>'&lt;table&gt;'</span><span>]</span><span>
     </span><span>for</span><span> last_name</span><span>,</span><span> first_name </span><span>in</span><span> employee_list</span><span>:</span><span>
         items</span><span>.</span><span>append</span><span>(</span><span>'&lt;tr&gt;&lt;td&gt;%s, %s&lt;/td&gt;&lt;/tr&gt;'</span><span> </span><span>%</span><span> </span><span>(</span><span>last_name</span><span>,</span><span> first_name</span><span>))</span><span>
     items</span><span>.</span><span>append</span><span>(</span><span>'&lt;/table&gt;'</span><span>)</span><span>
     employee_table </span><span>=</span><span> </span><span>''</span><span>.</span><span>join</span><span>(</span><span>items</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span> employee_table </span><span>=</span><span> </span><span>'&lt;table&gt;'</span><span>
    </span><span>for</span><span> last_name</span><span>,</span><span> first_name </span><span>in</span><span> employee_list</span><span>:</span><span>
        employee_table </span><span>+=</span><span> </span><span>'&lt;tr&gt;&lt;td&gt;%s, %s&lt;/td&gt;&lt;/tr&gt;'</span><span> </span><span>%</span><span> </span><span>(</span><span>last_name</span><span>,</span><span> first_name</span><span>)</span><span>
    employee_table </span><span>+=</span><span> </span><span>'&lt;/table&gt;'</span><span>
</span>
```

Be consistent with your choice of string quote character within a file. Pick `'` or `"` and stick with it. It is okay to use the other quote character on a string to avoid the need to `\\` escape within the string.

```
<span>Yes</span><span>:</span><span>
  </span><span>Python</span><span>(</span><span>'Why are you hiding your eyes?'</span><span>)</span><span>
  </span><span>Gollum</span><span>(</span><span>"I'm scared of lint errors."</span><span>)</span><span>
  </span><span>Narrator</span><span>(</span><span>'"Good!" thought a happy Python reviewer.'</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>
  </span><span>Python</span><span>(</span><span>"Why are you hiding your eyes?"</span><span>)</span><span>
  </span><span>Gollum</span><span>(</span><span>'The lint. It burns. It burns us.'</span><span>)</span><span>
  </span><span>Gollum</span><span>(</span><span>"Always the great lint. Watching. Watching."</span><span>)</span><span>
</span>
```

Prefer `"""` for multi-line strings rather than `'''`. Projects may choose to use `'''` for all non-docstring multi-line strings if and only if they also use `'` for regular strings. Docstrings must use `"""` regardless.

Multi-line strings do not flow with the indentation of the rest of the program. If you need to avoid embedding extra space in the string, use either concatenated single-line strings or a multi-line string with [`textwrap.dedent()`](https://docs.python.org/3/library/textwrap.html#textwrap.dedent) to remove the initial space on each line:

```
<span>  </span><span>No</span><span>:</span><span>
  long_string </span><span>=</span><span> </span><span>"""This is pretty ugly.
Don't do this.
"""</span><span>
</span>
```

```
<span>  </span><span>Yes</span><span>:</span><span>
  long_string </span><span>=</span><span> </span><span>"""This is fine if your use case can accept
      extraneous leading spaces."""</span><span>
</span>
```

```
<span>  </span><span>Yes</span><span>:</span><span>
  long_string </span><span>=</span><span> </span><span>(</span><span>"And this is fine if you can not accept\n"</span><span> </span><span>+</span><span>
                 </span><span>"extraneous leading spaces."</span><span>)</span><span>
</span>
```

```
<span>  </span><span>Yes</span><span>:</span><span>
  long_string </span><span>=</span><span> </span><span>(</span><span>"And this too is fine if you can not accept\n"</span><span>
                 </span><span>"extraneous leading spaces."</span><span>)</span><span>
</span>
```

```
<span>  </span><span>Yes</span><span>:</span><span>
  </span><span>import</span><span> textwrap

  long_string </span><span>=</span><span> textwrap</span><span>.</span><span>dedent</span><span>(</span><span>"""\
      This is also fine, because textwrap.dedent()
      will collapse common leading spaces in each line."""</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_11-Files-and-Sockets)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_11-files-and-sockets)3.11 Files and Sockets

Explicitly close files and sockets when done with them.

Leaving files, sockets or other file-like objects open unnecessarily has many downsides:

-   They may consume limited system resources, such as file descriptors. Code that deals with many such objects may exhaust those resources unnecessarily if they're not returned to the system promptly after use.
-   Holding files open may prevent other actions such as moving or deleting them.
-   Files and sockets that are shared throughout a program may inadvertently be read from or written to after logically being closed. If they are actually closed, attempts to read or write from them will throw exceptions, making the problem known sooner.

Furthermore, while files and sockets are automatically closed when the file object is destructed, tying the lifetime of the file object to the state of the file is poor practice:

-   There are no guarantees as to when the runtime will actually run the file‘s destructor. Different Python implementations use different memory management techniques, such as delayed Garbage Collection, which may increase the object’s lifetime arbitrarily and indefinitely.
-   Unexpected references to the file, e.g. in globals or exception tracebacks, may keep it around longer than intended.

The preferred way to manage files is using the [“with” statement](http://docs.python.org/reference/compound_stmts.html#the-with-statement):

```
<span>with</span><span> open</span><span>(</span><span>"hello.txt"</span><span>)</span><span> </span><span>as</span><span> hello_file</span><span>:</span><span>
    </span><span>for</span><span> line </span><span>in</span><span> hello_file</span><span>:</span><span>
        </span><span>print</span><span>(</span><span>line</span><span>)</span><span>
</span>
```

For file-like objects that do not support the “with” statement, use `contextlib.closing()`:

```
<span>import</span><span> contextlib

</span><span>with</span><span> contextlib</span><span>.</span><span>closing</span><span>(</span><span>urllib</span><span>.</span><span>urlopen</span><span>(</span><span>"http://www.python.org/"</span><span>))</span><span> </span><span>as</span><span> front_page</span><span>:</span><span>
    </span><span>for</span><span> line </span><span>in</span><span> front_page</span><span>:</span><span>
        </span><span>print</span><span>(</span><span>line</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_12-TODO-Comments)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_12-todo-comments)3.12 TODO Comments

Use `TODO` comments for code that is temporary, a short-term solution, or good-enough but not perfect.

A `TODO` comment begins with the string `TODO` in all caps and a parenthesized name, e-mail address, or other identifier of the person or issue with the best context about the problem. This is followed by an explanation of what there is to do.

The purpose is to have a consistent `TODO` format that can be searched to find out how to get more details. A `TODO` is not a commitment that the person referenced will fix the problem. Thus when you create a `TODO`, it is almost always your name that is given.

```
<span># TODO(kl@gmail.com): Use a "*" here for string repetition.</span><span>
</span><span># TODO(Zeke) Change this to use relations.</span><span>
</span>
```

If your `TODO` is of the form “At a future date do something” make sure that you either include a very specific date (“Fix by November 2009”) or a very specific event (“Remove this code when all clients can handle XML responses.”).

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_13-Imports-formatting)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_13-imports-formatting)3.13 Imports formatting

Imports should be on separate lines.

E.g.:

```
<span>Yes</span><span>:</span><span> </span><span>import</span><span> os
     </span><span>import</span><span> sys
</span>
```

```
<span>No</span><span>:</span><span>  </span><span>import</span><span> os</span><span>,</span><span> sys
</span>
```

Imports are always put at the top of the file, just after any module comments and docstrings and before module globals and constants. Imports should be grouped from most generic to least generic:

1.  Python future import statements. For example:

    ```
    <span>from</span><span> __future__ </span><span>import</span><span> absolute_import
    </span><span>from</span><span> __future__ </span><span>import</span><span> division
    </span><span>from</span><span> __future__ </span><span>import</span><span> print_function
    </span>
    ```

    See [above](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#from-future-imports) for more information about those.

2.  Python standard library imports. For example:

    ```
    <span>import</span><span> sys
    </span>
    ```

3.  [third-party](https://pypi.org/) module or package imports. For example:

    ```
    <span>import</span><span> tensorflow </span><span>as</span><span> tf
    </span>
    ```

4.  Code repository sub-package imports. For example:

    ```
    <span>from</span><span> otherproject</span><span>.</span><span>ai </span><span>import</span><span> mind
    </span>
    ```

5.  **Deprecated:** application-specific imports that are part of the same top level sub-package as this file. For example:

    ```
    <span>from</span><span> myproject</span><span>.</span><span>backend</span><span>.</span><span>hgwells </span><span>import</span><span> time_machine
    </span>
    ```

    You may find older Google Python Style code doing this, but it is no longer required. **New code is encouraged not to bother with this.** Simply treat application-specific sub-package imports the same as other sub-package imports.


Within each grouping, imports should be sorted lexicographically, ignoring case, according to each module's full package path. Code may optionally place a blank line between import sections.

```
<span>import</span><span> collections
</span><span>import</span><span> queue
</span><span>import</span><span> sys

</span><span>from</span><span> absl </span><span>import</span><span> app
</span><span>from</span><span> absl </span><span>import</span><span> flags
</span><span>import</span><span> bs4
</span><span>import</span><span> cryptography
</span><span>import</span><span> tensorflow </span><span>as</span><span> tf

</span><span>from</span><span> book</span><span>.</span><span>genres </span><span>import</span><span> scifi
</span><span>from</span><span> myproject</span><span>.</span><span>backend</span><span>.</span><span>hgwells </span><span>import</span><span> time_machine
</span><span>from</span><span> myproject</span><span>.</span><span>backend</span><span>.</span><span>state_machine </span><span>import</span><span> main_loop
</span><span>from</span><span> otherproject</span><span>.</span><span>ai </span><span>import</span><span> body
</span><span>from</span><span> otherproject</span><span>.</span><span>ai </span><span>import</span><span> mind
</span><span>from</span><span> otherproject</span><span>.</span><span>ai </span><span>import</span><span> soul

</span><span># Older style code may have these imports down here instead:</span><span>
</span><span>#from myproject.backend.hgwells import time_machine</span><span>
</span><span>#from myproject.backend.state_machine import main_loop</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_14-Statements)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_14-statements)3.14 Statements

Generally only one statement per line.

However, you may put the result of a test on the same line as the test only if the entire statement fits on one line. In particular, you can never do so with `try`/`except` since the `try` and `except` can't both fit on the same line, and you can only do so with an `if` if there is no `else`.

```
<span>Yes</span><span>:</span><span>

  </span><span>if</span><span> foo</span><span>:</span><span> bar</span><span>(</span><span>foo</span><span>)</span><span>
</span>
```

```
<span>No</span><span>:</span><span>

  </span><span>if</span><span> foo</span><span>:</span><span> bar</span><span>(</span><span>foo</span><span>)</span><span>
  </span><span>else</span><span>:</span><span>   baz</span><span>(</span><span>foo</span><span>)</span><span>

  </span><span>try</span><span>:</span><span>               bar</span><span>(</span><span>foo</span><span>)</span><span>
  </span><span>except</span><span> </span><span>ValueError</span><span>:</span><span> baz</span><span>(</span><span>foo</span><span>)</span><span>

  </span><span>try</span><span>:</span><span>
      bar</span><span>(</span><span>foo</span><span>)</span><span>
  </span><span>except</span><span> </span><span>ValueError</span><span>:</span><span> baz</span><span>(</span><span>foo</span><span>)</span><span>
</span>
```

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_15-Accessors)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_15-accessors)3.15 Accessors

If an accessor function would be trivial, you should use public variables instead of accessor functions to avoid the extra cost of function calls in Python. When more functionality is added you can use `property` to keep the syntax consistent.

On the other hand, if access is more complex, or the cost of accessing the variable is significant, you should use function calls (following the [Naming](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#s3.16-naming) guidelines) such as `get_foo()` and `set_foo()`. If the past behavior allowed access through a property, do not bind the new accessor functions to the property. Any code still attempting to access the variable by the old method should break visibly so they are made aware of the change in complexity.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16-Naming)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16-naming)3.16 Naming

`module_name`, `package_name`, `ClassName`, `method_name`, `ExceptionName`, `function_name`, `GLOBAL_CONSTANT_NAME`, `global_var_name`, `instance_var_name`, `function_parameter_name`, `local_var_name`.

Function names, variable names, and filenames should be descriptive; eschew abbreviation. In particular, do not use abbreviations that are ambiguous or unfamiliar to readers outside your project, and do not abbreviate by deleting letters within a word.

Always use a `.py` filename extension. Never use dashes.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_1-Names-to-Avoid)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_1-names-to-avoid)3.16.1 Names to Avoid

-   single character names except for counters or iterators. You may use “e” as an exception identifier in try/except statements.
-   dashes (`-`) in any package/module name
-   `__double_leading_and_trailing_underscore__` names (reserved by Python)

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_2-Naming-Conventions)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_2-naming-conventions)3.16.2 Naming Conventions

-   “Internal” means internal to a module, or protected or private within a class.

-   Prepending a single underscore (`_`) has some support for protecting module variables and functions (not included with `from module import *`). While prepending a double underscore (`__` aka “dunder”) to an instance variable or method effectively makes the variable or method private to its class (using name mangling) we discourage its use as it impacts readability and testability and isn't _really_ private.

-   Place related classes and top-level functions together in a module. Unlike Java, there is no need to limit yourself to one class per module.

-   Use CapWords for class names, but lower\_with\_under.py for module names. Although there are some old modules named CapWords.py, this is now discouraged because it's confusing when the module happens to be named after a class. (“wait -- did I write `import StringIO` or `from StringIO import StringIO`?”)

-   Underscores may appear in _unittest_ method names starting with `test` to separate logical components of the name, even if those components use CapWords. One possible pattern is `test<MethodUnderTest>_<state>`; for example `testPop_EmptyStack` is okay. There is no One Correct Way to name test methods.


#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_3-File-Naming)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_3-file-naming)3.16.3 File Naming

Python filenames must have a `.py` extension and must not contain dashes (`-`). This allows them to be imported and unittested. If you want an executable to be accessible without the extension, use a symbolic link or a simple bash wrapper containing `exec "$0.py" "$@"`.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_4-Guidelines-derived-from-Guido_s-Recommendations)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_16_4-guidelines-derived-from-guido_s-recommendations)3.16.4 Guidelines derived from Guido's Recommendations

While Python supports making things private by using a leading double underscore `__` (aka. “dunder”) prefix on a name, this is discouraged. Prefer the use of a single underscore. They are easier to type, read, and to access from small unittests. Lint warnings take care of invalid access to protected members.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_17-Main)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_17-main)3.17 Main

Even a file meant to be used as an executable should be importable and a mere import should not have the side effect of executing the program's main functionality. The main functionality should be in a `main()` function.

In Python, `pydoc` as well as unit tests require modules to be importable. Your code should always check `if __name__ == '__main__'` before executing your main program so that the main program is not executed when the module is imported.

```
<span>def</span><span> main</span><span>():</span><span>
    </span><span>...</span><span>

</span><span>if</span><span> __name__ </span><span>==</span><span> </span><span>'__main__'</span><span>:</span><span>
    main</span><span>()</span><span>
</span>
```

All code at the top level will be executed when the module is imported. Be careful not to call functions, create objects, or perform other operations that should not be executed when the file is being `pydoc`ed.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_18-Function-length)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_18-function-length)3.18 Function length

Prefer small and focused functions.

We recognize that long functions are sometimes appropriate, so no hard limit is placed on function length. If a function exceeds about 40 lines, think about whether it can be broken up without harming the structure of the program.

Even if your long function works perfectly now, someone modifying it in a few months may add new behavior. This could result in bugs that are hard to find. Keeping your functions short and simple makes it easier for other people to read and modify your code.

You could find long and complicated functions when working with some code. Do not be intimidated by modifying existing code: if working with such a function proves to be difficult, you find that errors are hard to debug, or you want to use a piece of it in several different contexts, consider breaking up the function into smaller and more manageable pieces.

### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19-Type-Annotations)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19-type-annotations)3.19 Type Annotations

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_1-General-Rules)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_1-general-rules)3.19.1 General Rules

-   Familiarize yourself with [PEP-484](https://www.python.org/dev/peps/pep-0484/).
-   In methods, only annotate `self`, or `cls` if it is necessary for proper type information. e.g., `@classmethod def create(cls: Type[T]) -> T: return cls()`
-   If any other variable or a returned type should not be expressed, use `Any`.
-   You are not required to annotate all the functions in a module.
    -   At least annotate your public APIs.
    -   Use judgment to get to a good balance between safety and clarity on the one hand, and flexibility on the other.
    -   Annotate code that is prone to type-related errors (previous bugs or complexity).
    -   Annotate code that is hard to understand.
    -   Annotate code as it becomes stable from a types perspective. In many cases, you can annotate all the functions in mature code without losing too much flexibility.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_2-Line-Breaking)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_2-line-breaking)3.19.2 Line Breaking

Try to follow the existing [indentation](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#indentation) rules.

After annotating, many function signatures will become “one parameter per line”.

```
<span>def</span><span> my_method</span><span>(</span><span>self</span><span>,</span><span>
              first_var</span><span>:</span><span> int</span><span>,</span><span>
              second_var</span><span>:</span><span> </span><span>Foo</span><span>,</span><span>
              third_var</span><span>:</span><span> </span><span>Optional</span><span>[</span><span>Bar</span><span>])</span><span> </span><span>-&gt;</span><span> int</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

Always prefer breaking between variables, and not for example between variable names and type annotations. However, if everything fits on the same line, go for it.

```
<span>def</span><span> my_method</span><span>(</span><span>self</span><span>,</span><span> first_var</span><span>:</span><span> int</span><span>)</span><span> </span><span>-&gt;</span><span> int</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

If the combination of the function name, the last parameter, and the return type is too long, indent by 4 in a new line.

```
<span>def</span><span> my_method</span><span>(</span><span>
    self</span><span>,</span><span> first_var</span><span>:</span><span> int</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Tuple</span><span>[</span><span>MyLongType1</span><span>,</span><span> </span><span>MyLongType1</span><span>]:</span><span>
  </span><span>...</span><span>
</span>
```

When the return type does not fit on the same line as the last parameter, the preferred way is to indent the parameters by 4 on a new line and align the closing parenthesis with the def.

```
<span>Yes</span><span>:</span><span>
</span><span>def</span><span> my_method</span><span>(</span><span>
    self</span><span>,</span><span> other_arg</span><span>:</span><span> </span><span>Optional</span><span>[</span><span>MyLongType</span><span>]</span><span>
</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>[</span><span>OtherLongType</span><span>,</span><span> </span><span>MyLongType</span><span>]:</span><span>
  </span><span>...</span><span>
</span>
```

`pylint` allows you to move the closing parenthesis to a new line and align with the opening one, but this is less readable.

```
<span>No</span><span>:</span><span>
</span><span>def</span><span> my_method</span><span>(</span><span>self</span><span>,</span><span>
              other_arg</span><span>:</span><span> </span><span>Optional</span><span>[</span><span>MyLongType</span><span>]</span><span>
             </span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>[</span><span>OtherLongType</span><span>,</span><span> </span><span>MyLongType</span><span>]:</span><span>
  </span><span>...</span><span>
</span>
```

As in the examples above, prefer not to break types. However, sometimes they are too long to be on a single line (try to keep sub-types unbroken).

```
<span>def</span><span> my_method</span><span>(</span><span>
    self</span><span>,</span><span>
    first_var</span><span>:</span><span> </span><span>Tuple</span><span>[</span><span>List</span><span>[</span><span>MyLongType1</span><span>],</span><span>
                     </span><span>List</span><span>[</span><span>MyLongType2</span><span>]],</span><span>
    second_var</span><span>:</span><span> </span><span>List</span><span>[</span><span>Dict</span><span>[</span><span>
        </span><span>MyLongType3</span><span>,</span><span> </span><span>MyLongType4</span><span>]])</span><span> </span><span>-&gt;</span><span> </span><span>None</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

If a single name and type is too long, consider using an [alias](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-aliases) for the type. The last resort is to break after the colon and indent by 4.

```
<span>Yes</span><span>:</span><span>
</span><span>def</span><span> my_function</span><span>(</span><span>
    long_variable_name</span><span>:</span><span>
        long_module_name</span><span>.</span><span>LongTypeName</span><span>,</span><span>
</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>None</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

```
<span>No</span><span>:</span><span>
</span><span>def</span><span> my_function</span><span>(</span><span>
    long_variable_name</span><span>:</span><span> long_module_name</span><span>.</span><span>
        </span><span>LongTypeName</span><span>,</span><span>
</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>None</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_3-Forward-Declarations)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_3-forward-declarations)3.19.3 Forward Declarations

If you need to use a class name from the same module that is not yet defined -- for example, if you need the class inside the class declaration, or if you use a class that is defined below -- use a string for the class name.

```
<span>class</span><span> </span><span>MyClass</span><span>(</span><span>object</span><span>):</span><span>

  </span><span>def</span><span> __init__</span><span>(</span><span>self</span><span>,</span><span>
               stack</span><span>:</span><span> </span><span>List</span><span>[</span><span>"MyClass"</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>None</span><span>:</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_4-Default-Values)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_4-default-values)3.19.4 Default Values

As per [PEP-008](https://www.python.org/dev/peps/pep-0008/#other-recommendations), use spaces around the `=` _only_ for arguments that have both a type annotation and a default value.

```
<span>Yes</span><span>:</span><span>
</span><span>def</span><span> func</span><span>(</span><span>a</span><span>:</span><span> int </span><span>=</span><span> </span><span>0</span><span>)</span><span> </span><span>-&gt;</span><span> int</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

```
<span>No</span><span>:</span><span>
</span><span>def</span><span> func</span><span>(</span><span>a</span><span>:</span><span>int</span><span>=</span><span>0</span><span>)</span><span> </span><span>-&gt;</span><span> int</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_5-NoneType)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_5-nonetype)3.19.5 NoneType

In the Python type system, `NoneType` is a “first class” type, and for typing purposes, `None` is an alias for `NoneType`. If an argument can be `None`, it has to be declared! You can use `Union`, but if there is only one other type, use `Optional`.

Use explicit `Optional` instead of implicit `Optional`. Earlier versions of PEP 484 allowed `a: Text = None` to be interpretted as `a: Optional[Text] = None`, but that is no longer the preferred behavior.

```
<span>Yes</span><span>:</span><span>
</span><span>def</span><span> func</span><span>(</span><span>a</span><span>:</span><span> </span><span>Optional</span><span>[</span><span>Text</span><span>],</span><span> b</span><span>:</span><span> </span><span>Optional</span><span>[</span><span>Text</span><span>]</span><span> </span><span>=</span><span> </span><span>None</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Text</span><span>:</span><span>
  </span><span>...</span><span>
</span><span>def</span><span> multiple_nullable_union</span><span>(</span><span>a</span><span>:</span><span> </span><span>Union</span><span>[</span><span>None</span><span>,</span><span> </span><span>Text</span><span>,</span><span> int</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Text</span><span>
  </span><span>...</span><span>
</span>
```

```
<span>No</span><span>:</span><span>
</span><span>def</span><span> nullable_union</span><span>(</span><span>a</span><span>:</span><span> </span><span>Union</span><span>[</span><span>None</span><span>,</span><span> </span><span>Text</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Text</span><span>:</span><span>
  </span><span>...</span><span>
</span><span>def</span><span> implicit_optional</span><span>(</span><span>a</span><span>:</span><span> </span><span>Text</span><span> </span><span>=</span><span> </span><span>None</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Text</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_6-Type-Aliases)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_6-type-aliases)3.19.6 Type Aliases

You can declare aliases of complex types. The name of an alias should be CapWorded. If the alias is used only in this module, it should be \_Private.

For example, if the name of the module together with the name of the type is too long:

```
<span>_ShortName</span><span> </span><span>=</span><span> module_with_long_name</span><span>.</span><span>TypeWithLongName</span><span>
</span><span>ComplexMap</span><span> </span><span>=</span><span> </span><span>Mapping</span><span>[</span><span>Text</span><span>,</span><span> </span><span>List</span><span>[</span><span>Tuple</span><span>[</span><span>int</span><span>,</span><span> int</span><span>]]]</span><span>
</span>
```

Other examples are complex nested types and multiple return variables from a function (as a tuple).

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_7-Ignoring-Types)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_7-ignoring-types)3.19.7 Ignoring Types

You can disable type checking on a line with the special comment `# type: ignore`.

`pytype` has a disable option for specific errors (similar to lint):

```
<span># pytype: disable=attribute-error</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_8-Typing-Variables)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_8-typing-variables)3.19.8 Typing Variables

If an internal variable has a type that is hard or impossible to infer, you can specify its type in a couple ways.

[_Type Comments:_](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#type-comments) : Use a `# type:` comment on the end of the line

````
```python
a = SomeUndecoratedFunction()  # type: Foo
```
````

[_Annotated Assignments_](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#annotated-assignments) : Use a colon and type between the variable name and value, as with function arguments.

````
```python
a: Foo = SomeUndecoratedFunction()
```
````

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_9-Tuples-vs-Lists)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_9-tuples-vs-lists)3.19.9 Tuples vs Lists

Unlike Lists, which can only have a single type, Tuples can have either a single repeated type or a set number of elements with different types. The latter is commonly used as return type from a function.

```
<span>a </span><span>=</span><span> </span><span>[</span><span>1</span><span>,</span><span> </span><span>2</span><span>,</span><span> </span><span>3</span><span>]</span><span>  </span><span># type: List[int]</span><span>
b </span><span>=</span><span> </span><span>(</span><span>1</span><span>,</span><span> </span><span>2</span><span>,</span><span> </span><span>3</span><span>)</span><span>  </span><span># type: Tuple[int, ...]</span><span>
c </span><span>=</span><span> </span><span>(</span><span>1</span><span>,</span><span> </span><span>"2"</span><span>,</span><span> </span><span>3.5</span><span>)</span><span>  </span><span># type: Tuple[int, Text, float]</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_10-TypeVars)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_10-typevars)3.19.10 TypeVars

The Python type system has [generics](https://www.python.org/dev/peps/pep-0484/#generics). The factory function `TypeVar` is a common way to use them.

Example:

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>List</span><span>,</span><span> </span><span>TypeVar</span><span>
T </span><span>=</span><span> </span><span>TypeVar</span><span>(</span><span>"T"</span><span>)</span><span>
</span><span>...</span><span>
</span><span>def</span><span> next</span><span>(</span><span>l</span><span>:</span><span> </span><span>List</span><span>[</span><span>T</span><span>])</span><span> </span><span>-&gt;</span><span> T</span><span>:</span><span>
  </span><span>return</span><span> l</span><span>.</span><span>pop</span><span>()</span><span>
</span>
```

A TypeVar can be constrained:

```
<span>AddableType</span><span> </span><span>=</span><span> </span><span>TypeVar</span><span>(</span><span>"AddableType"</span><span>,</span><span> int</span><span>,</span><span> float</span><span>,</span><span> </span><span>Text</span><span>)</span><span>
</span><span>def</span><span> add</span><span>(</span><span>a</span><span>:</span><span> </span><span>AddableType</span><span>,</span><span> b</span><span>:</span><span> </span><span>AddableType</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>AddableType</span><span>:</span><span>
  </span><span>return</span><span> a </span><span>+</span><span> b
</span>
```

A common predefined type variable in the `typing` module is `AnyStr`. Use it for multiple annotations that can be `bytes` or `unicode` and must all be the same type.

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>AnyStr</span><span>
</span><span>def</span><span> check_length</span><span>(</span><span>x</span><span>:</span><span> </span><span>AnyStr</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>AnyStr</span><span>:</span><span>
  </span><span>if</span><span> len</span><span>(</span><span>x</span><span>)</span><span> </span><span>&lt;=</span><span> </span><span>42</span><span>:</span><span>
    </span><span>return</span><span> x
  </span><span>raise</span><span> </span><span>ValueError</span><span>()</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_11-String-types)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_11-string-types)3.19.11 String types

The proper type for annotating strings depends on what versions of Python the code is intended for.

For Python 3 only code, prefer to use `str`. `Text` is also acceptable. Be consistent in using one or the other.

For Python 2 compatible code, use `Text`. In some rare cases, `str` may make sense; typically to aid compatibility when the return types aren‘t the same between the two Python versions. Avoid using `unicode`: it doesn’t exist in Python 3.

The reason this discrepancy exists is because `str` means different things depending on the Python version.

```
<span>No</span><span>:</span><span>
</span><span>def</span><span> py2_code</span><span>(</span><span>x</span><span>:</span><span> str</span><span>)</span><span> </span><span>-&gt;</span><span> unicode</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

For code that deals with binary data, use `bytes`.

```
<span>def</span><span> deals_with_binary_data</span><span>(</span><span>x</span><span>:</span><span> bytes</span><span>)</span><span> </span><span>-&gt;</span><span> bytes</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

For Python 2 compatible code that processes text data (`str` or `unicode` in Python 2, `str` in Python 3), use `Text`. For Python 3 only code that process text data, prefer `str`.

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>Text</span><span>
</span><span>...</span><span>
</span><span>def</span><span> py2_compatible</span><span>(</span><span>x</span><span>:</span><span> </span><span>Text</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Text</span><span>:</span><span>
  </span><span>...</span><span>
</span><span>def</span><span> py3_only</span><span>(</span><span>x</span><span>:</span><span> str</span><span>)</span><span> </span><span>-&gt;</span><span> str</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

If the type can be either bytes or text, use `Union`, with the appropriate text type.

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>Text</span><span>,</span><span> </span><span>Union</span><span>
</span><span>...</span><span>
</span><span>def</span><span> py2_compatible</span><span>(</span><span>x</span><span>:</span><span> </span><span>Union</span><span>[</span><span>bytes</span><span>,</span><span> </span><span>Text</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Union</span><span>[</span><span>bytes</span><span>,</span><span> </span><span>Text</span><span>]:</span><span>
  </span><span>...</span><span>
</span><span>def</span><span> py3_only</span><span>(</span><span>x</span><span>:</span><span> </span><span>Union</span><span>[</span><span>bytes</span><span>,</span><span> str</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Union</span><span>[</span><span>bytes</span><span>,</span><span> str</span><span>]:</span><span>
  </span><span>...</span><span>
</span>
```

If all the string types of a function are always the same, for example if the return type is the same as the argument type in the code above, use [AnyStr](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-type-var).

Writing it like this will simplify the process of porting the code to Python 3.

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_12-Imports-For-Typing)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_12-imports-for-typing)3.19.12 Imports For Typing

For classes from the `typing` module, always import the class itself. You are explicitly allowed to import multiple specific classes on one line from the `typing` module. Ex:

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>Any</span><span>,</span><span> </span><span>Dict</span><span>,</span><span> </span><span>Optional</span><span>
</span>
```

Given that this way of importing from `typing` adds items to the local namespace, any names in `typing` should be treated similarly to keywords, and not be defined in your Python code, typed or not. If there is a collision between a type and an existing name in a module, import it using `import x as y`.

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>Any</span><span> </span><span>as</span><span> </span><span>AnyType</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_13-Conditional-Imports)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_13-conditional-imports)3.19.13 Conditional Imports

Use conditional imports only in exceptional cases where the additional imports needed for type checking must be avoided at runtime. This pattern is discouraged; alternatives such as refactoring the code to allow top level imports should be preferred.

Imports that are needed only for type annotations can be placed within an `if TYPE_CHECKING:` block.

-   Conditionally imported types need to be referenced as strings, to be forward compatible with Python 3.6 where the annotation expressions are actually evaluated.
-   Only entities that are used solely for typing should be defined here; this includes aliases. Otherwise it will be a runtime error, as the module will not be imported at runtime.
-   The block should be right after all the normal imports.
-   There should be no empty lines in the typing imports list.
-   Sort this list as if it were a regular imports list.

```
<span>import</span><span> typing
</span><span>if</span><span> typing</span><span>.</span><span>TYPE_CHECKING</span><span>:</span><span>
  </span><span>import</span><span> sketch
</span><span>def</span><span> f</span><span>(</span><span>x</span><span>:</span><span> </span><span>"sketch.Sketch"</span><span>):</span><span> </span><span>...</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_14-Circular-Dependencies)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_14-circular-dependencies)3.19.14 Circular Dependencies

Circular dependencies that are caused by typing are code smells. Such code is a good candidate for refactoring. Although technically it is possible to keep circular dependencies, the [build system](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-build-deps) will not let you do so because each module has to depend on the other.

Replace modules that create circular dependency imports with `Any`. Set an [alias](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-aliases) with a meaningful name, and use the real type name from this module (any attribute of Any is Any). Alias definitions should be separated from the last import by one line.

```
<span>from</span><span> typing </span><span>import</span><span> </span><span>Any</span><span>

some_mod </span><span>=</span><span> </span><span>Any</span><span>  </span><span># some_mod.py imports this module.</span><span>
</span><span>...</span><span>

</span><span>def</span><span> my_method</span><span>(</span><span>self</span><span>,</span><span> var</span><span>:</span><span> some_mod</span><span>.</span><span>SomeType</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>None</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

#### [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_15-Generics)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#3_19_15-generics)3.19.15 Generics

When annotating, prefer to specify type parameters for generic types; otherwise, [the generics' parameters will be assumed to be `Any`](https://www.python.org/dev/peps/pep-0484/#the-any-type).

```
<span>def</span><span> get_names</span><span>(</span><span>employee_ids</span><span>:</span><span> </span><span>List</span><span>[</span><span>int</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>[</span><span>int</span><span>,</span><span> </span><span>Any</span><span>]:</span><span>
  </span><span>...</span><span>
</span>
```

```
<span># These are both interpreted as get_names(employee_ids: List[Any]) -&gt; Dict[Any, Any]</span><span>
</span><span>def</span><span> get_names</span><span>(</span><span>employee_ids</span><span>:</span><span> list</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>:</span><span>
  </span><span>...</span><span>

</span><span>def</span><span> get_names</span><span>(</span><span>employee_ids</span><span>:</span><span> </span><span>List</span><span>)</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>:</span><span>
  </span><span>...</span><span>
</span>
```

If the best type parameter for a generic is `Any`, make it explicit, but remember that in many cases [`TypeVar`](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#typing-type-var) might be more appropriate:

```
<span>def</span><span> get_names</span><span>(</span><span>employee_ids</span><span>:</span><span> </span><span>List</span><span>[</span><span>Any</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>[</span><span>Any</span><span>,</span><span> </span><span>Text</span><span>]:</span><span>
  </span><span>"""Returns a mapping from employee ID to employee name for given IDs."""</span><span>
</span>
```

```
<span>T </span><span>=</span><span> </span><span>TypeVar</span><span>(</span><span>'T'</span><span>)</span><span>
</span><span>def</span><span> get_names</span><span>(</span><span>employee_ids</span><span>:</span><span> </span><span>List</span><span>[</span><span>T</span><span>])</span><span> </span><span>-&gt;</span><span> </span><span>Dict</span><span>[</span><span>T</span><span>,</span><span> </span><span>Text</span><span>]:</span><span>
  </span><span>"""Returns a mapping from employee ID to employee name for given IDs."""</span><span>
</span>
```

## [](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#4-Parting-Words)[](https://android.googlesource.com/platform/external/google-styleguide/+/refs/tags/android-s-beta-2/pyguide.md#4-parting-words)4 Parting Words

_BE CONSISTENT_.

If you're editing code, take a few minutes to look at the code around you and determine its style. If they use spaces around all their arithmetic operators, you should too. If their comments have little boxes of hash marks around them, make your comments have little boxes of hash marks around them too.

The point of having style guidelines is to have a common vocabulary of coding so people can concentrate on what you‘re saying rather than on how you’re saying it. We present global style rules here so people know the vocabulary, but local style is also important. If code you add to a file looks drastically different from the existing code around it, it throws readers out of their rhythm when they go to read it. Avoid this.
