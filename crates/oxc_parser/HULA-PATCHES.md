# Hula patches to oxc_parser

Upstream: https://github.com/oxc-project/oxc, revision 0389c4010ef8298728f3e47fff8e2a9106d10045, path `crates/oxc_parser`.
Base: crates.io `oxc_parser` 0.146.0, `.crate` SHA-256 311c29dfdf55ea8bf065cf300b3d0dca5fe1c0fb8059c9ba70a6c98cce75306e.
`LICENSE` is the upstream repository license at the same revision.
The base commit contains the published crate without changes. Contract: `specs/contracts/p2/pipeline.md`.

## Patches

### Nesting guard (staged, 2026-10-08)

Problem: a nesting guard cannot bound stack usage through unguarded declaration dispatch.
The guard bounds nesting depth. The later compiler thread supplies at least 16 MiB of stack.
Its stack must also exceed eight times the worst debug mark. No host-thread stack size is assumed.
Contract: the amended working-tree `specs/contracts/p2/pipeline.md`, Nesting guard and Compiler thread.

`ParseOptions::max_nesting_depth: Option<usize>` enables the guard. Its default is `None`.
`ParserReturn::depth_exceeded: Option<u32>` records the current token's byte offset when the exceeded guard is entered.
The offset does not identify a uniform syntax opener. Assignment reports `=`; binary RHS entry reports the following token.
A latched failure preserves that first offset through recovery and speculation.
Depth exhaustion sets the existing fatal error, empties the program, and returns `panicked = true`.
The diagnostic reads `Parser nesting depth exceeded`. `parse_expression()` returns that diagnostic through `Err`.
Owned RAII guards increment on entry and decrement on every exit, including early returns and unwinding.
The counter is independent of checkpoints. Rewind cannot discard a latched depth failure.
Leaves and sequential siblings do not consume additional nesting depth.

Guard concrete recursive constructs instead of every precedence or statement dispatcher.
Dispatcher guards would count ordinary dispatch frames multiple times and reject 128-level syntax prematurely.
Guards cover parameter initializers as well as function bodies, speculative arrows, and recursive type lookahead.
PURE annotation and JSX member-name comparison now use loops because their inputs can contain unlimited iterative chains.

Optional regular-expression AST parsing enters an unmodified dependency with larger recursive frames.
Its entry checks unescaped groups and Unicode-set classes iteratively, using the same counter and RAII guards.
Each regex level reserves four counter units. A maximum of 128 therefore permits 32 regex levels without surrounding nesting.
Escaped delimiters and parentheses inside character classes do not count.
Regex exhaustion reports the regex literal token's offset, even though lexing already advanced past that token.
Evidence: 128 regex levels aborted; 96 namespace levels plus 32 regex groups also aborted on 1 MiB.
The reservation does not guarantee stack safety on arbitrary 1 MiB threads.
The default tests use 16 MiB, including group, lookaround, Unicode-set, and namespace/regex combinations.
The review measured 32 groups at 659,456 bytes in debug.
That leaves 16,117,760 bytes of the 16 MiB reservation; the reservation is about 25.44 times this mark.
Default regex AST parsing remains disabled. Disabling the nesting guard preserves the upstream option behavior.

### Recursive-cycle coverage

Each row names a cycle family and every guarded function covering that family.
Cross-family cycles encounter at least one of their constituent guards.
Removing guarded vertices and guarded recursive edges leaves an acyclic parser-method graph, including `Self::` callbacks.
Cover-grammar trait dispatch is audited separately. Lexer scanning and module-record loops do not introduce parser recursion.
The two AST-only recursive traversals are eliminated, rather than charged against a counter after parsing.

| Recursive cycle family | Guarded function or edge | Coverage |
|---|---|---|
| Parentheses -> expression -> parentheses | `parse_parenthesized_expression` | Every parenthesized primary. |
| Array -> element -> assignment expression -> array | `parse_array_expression` | Elements and nested spreads return through this entry. |
| Object -> property/value/computed key -> expression -> object | `parse_object_expression` | Includes methods, defaults, and computed keys. |
| Template -> substitution -> expression -> template | `parse_template_literal` | Both ordinary and tagged substitutions. |
| New -> primary/member -> new | `parse_new_expression` | Includes recursive constructors and constructor arguments. |
| Unary -> simple unary -> unary | `parse_unary_expression` | Covers `!`, `-`, `typeof`, and other unary operators. |
| Prefix update -> unary -> prefix update | `parse_update_expression`, prefix branch | Postfix updates do not recurse. |
| Binary rest -> binary RHS -> binary rest | `parse_binary_expression_rest`, RHS edge | Covers right-associative exponentiation and recursive precedence descent. |
| Private-in -> binary RHS -> private-in | `parse_private_in_expression` | Also bounds malformed nested private-in expressions. |
| Conditional -> consequent/alternate -> assignment -> conditional | `parse_conditional_expression_rest`, question branch | Leaves return before the guard. |
| Assignment -> right assignment -> assignment | `parse_assignment_expression_recursive` | Guard starts after target conversion, before parsing the RHS. |
| Await -> unary -> await | `parse_await_expression` | Includes unambiguous-module reparsing. |
| Yield -> assignment -> yield | `parse_yield_expression` | Includes delegated yields. |
| Computed member -> expression -> computed member | `parse_computed_member_expression` | Dot-member chains remain iterative. |
| Call -> argument -> assignment -> call | `parse_call_arguments` | Empty chained calls do not retain nested guards. |
| Import expression -> argument -> assignment -> import expression | `parse_import_expression` | Includes import options. |
| V8 intrinsic -> argument -> assignment -> V8 intrinsic | `parse_v8_intrinsic_expression` | Covers this opt-in parser extension. |
| Decorated expression -> decorator -> expression -> decorated expression | `parse_decorated_expression` | Also reaches the class guard. |
| Simple arrow -> body -> assignment -> simple arrow | `parse_simple_arrow_function_expression` | Guard surrounds both arrow construction and body. |
| Definite parenthesized arrow -> parameters/body -> arrow | `parse_parenthesized_arrow_function_expression` | Includes parameter initializers and return annotations. |
| Speculative parenthesized arrow -> parameters/body -> arrow | `parse_possible_parenthesized_arrow_function_expression` | Failed speculation cannot erase exhaustion. |
| Function -> parameters/body -> statement/expression -> function | `parse_function` | Declaration, expression, async, generator, and ambient entries converge here. |
| Method -> parameters/body -> expression -> method | `parse_function`, called directly by `parse_method` | The redundant method guard is removed. |
| Class -> heritage/member/body -> expression/statement -> class | `parse_class` | Covers nested heritage classes and static blocks. |
| Block -> statement list -> block | `parse_block` | Also covers try/catch/finally block cycles. |
| If -> consequent/alternate -> statement -> if | `parse_if_statement` | Both if and else chains. |
| Do -> statement body -> do | `parse_do_while_statement` | Body guard remains active through the trailing condition. |
| While -> statement body -> while | `parse_while_statement` | Includes non-block bodies. |
| For -> statement body -> for | `parse_for_statement` | Ordinary, in, of, and await forms converge here. |
| With -> statement body -> with | `parse_with_statement` | Also bounds syntax rejected in strict mode. |
| Switch -> case statement list -> switch | `parse_switch_statement` | Cases alone are iterative siblings. |
| Label -> statement -> label | `parse_expression_or_labeled_statement`, colon branch | Expression statements do not retain a label guard. |
| Array binding -> binding element/rest -> array binding | `parse_array_binding_pattern` | Includes nested rest/default patterns. |
| Object binding -> binding property/rest -> object binding | `parse_object_binding_pattern` | Includes nested computed properties and defaults. |
| Namespace -> dotted namespace/module block -> namespace | `parse_module_or_namespace_declaration` | Dotted namespace recursion and nested bodies. |
| External ambient module -> module block -> external module | `parse_ambient_external_module_declaration` | Separate entry from named namespaces. |
| Global declaration -> module block -> global declaration | `parse_ts_global_declaration` | Covers nested ambient globals, including invalid syntax. |
| Conditional type -> constraint/branches -> TS type | `parse_ts_type`, extends branch | Non-conditional leaf types remain uncharged. |
| Function/constructor type -> parameters/return -> TS type | `parse_function_or_constructor_type` | Generic, abstract, and constructor signatures converge here. |
| Type operator -> operator or higher -> type operator | `parse_type_operator` | Keyof, unique, and readonly. |
| Infer -> constraint -> infer/type | `parse_infer_type` | Includes speculative constraint rewinds. |
| Indexed type -> index TS type -> indexed type | `parse_postfix_type_or_higher`, indexed branch | Array suffixes are iterative. |
| Type lookahead -> parameter/type start -> parenthesized lookahead | `is_start_of_parenthesized_or_function_type` | Guards speculative recursion before AST construction. |
| Mapped type -> parameter/name/value type -> mapped type | `parse_mapped_type` | Includes type remapping. |
| Type literal -> signature/property type -> type literal | `parse_type_literal` | Covers interface signature recursion through nested types. |
| Template type -> substitution TS type -> template type | `parse_template_type` | Type-level template substitutions. |
| Tuple -> element type -> tuple | `parse_tuple_type` | Includes named, optional, and rest elements. |
| Parenthesized type -> TS type -> parenthesized type | `parse_parenthesized_type` | Shares the counter with expression parsing. |
| Generic reference -> argument type -> generic reference | `parse_type_arguments_of_type_reference` | Guard only when an argument list opens. |
| Generic expression -> argument type -> expression/type | `parse_type_arguments_in_expression` | Speculative argument-list entry. |
| Instantiation -> argument type -> instantiation/type | `try_parse_type_arguments` | Shared class, call, and heritage argument entry. |
| This predicate -> annotated type -> predicate/type | `parse_this_type_predicate` | Recursion can bypass composite type entries. |
| Asserts predicate -> annotated type -> predicate/type | `parse_asserts_type_predicate` | Includes recursively malformed predicates. |
| TS import type -> argument/options/type arguments -> type | `parse_ts_import_type` | Argument parsing can recurse independently of generic lists. |
| Nullable JSDoc type -> TS type -> nullable type | `parse_js_doc_unknown_or_nullable_type` | Covers the parser's accepted JSDoc-shaped type syntax. |
| Non-nullable JSDoc type -> non-array type -> non-nullable type | `parse_js_doc_non_nullable_type` | Prefix forms recurse; postfix forms are iterative. |
| TS assertion -> unary -> TS assertion | `parse_ts_type_assertion` | Angle-bracket assertions, including mixed assertions and unary operators. |
| JSX element -> child/attribute -> JSX element | `parse_jsx_element` | Includes attribute expressions and nested JSX values. |
| JSX fragment -> child -> JSX fragment | `parse_jsx_fragment` | Mixed element/fragment cycles cross one of these two guards. |
| Array cover -> target/default -> array cover | `ArrayAssignmentTarget::cover` | Conversion uses a separate bounded pass over the parsed tree. |
| Object cover -> property/target/default -> object cover | `ObjectAssignmentTarget::cover` | Covers spread targets and defaults. |
| Parenthesized cover -> simple target -> parenthesized cover | `SimpleAssignmentTarget::cover`, parentheses arm | Conversion cannot reintroduce an unguarded cycle. |
| PURE marking -> left/test/member/chain -> PURE marking | Eliminated in `set_pure_on_call_or_new_expr` | Loop preserves annotation behavior on long iterative chains. |
| JSX dotted-name comparison -> member object -> comparison | Eliminated in `jsx_member_expression_eq` | Loop bounds stack independently of dotted-name length. |
| External regex disjunction -> group/lookaround -> disjunction | `check_regex_nesting`, before `parse_regex_pattern` | Total preflight bounds the unmodified external cycle. |
| External regex class-set -> nested class -> class-set | `check_regex_nesting`, Unicode-set mode | Same bound covers nested `v` character classes. |

### Tests and effective source depth

`crates/hula-core/tests/parser_nesting.rs` owns the deterministic probes and subprocess stack measurement.
Each default parse uses a spawned 16 MiB thread, matching the planned compiler stack.
The boundary matrix covers 47 forms and 196 applicable form/wrapper pairs.
Wrappers include exports, ambient declarations, enum initializers, and destructuring defaults.
The four declaration-dispatch review inputs appear as repeated layers, not only outer wrappers.
Every recursive pair checks its accepted source-depth boundary, the next level, levels 128/129, and its largest source within 65,536 bytes.
Cases exceeding 128 guard units at source depth 128 must report exhaustion at that source depth.
All depth-129 recursive cases assert `depth_exceeded`, including every entry in `other_recursive_paths`.
Prefix update uses `++(`, and nullable type extensions use `?(`, instead of shallow malformed `++`/`??A` inputs.
Iterative unions, call chains, dot members, binary chains, and PURE-member chains remain accepted without depth failure.
The seeded LCG produces 2,000 mixed inputs, including inputs above 65,500 bytes.
It composes export, declare-type, enum-initializer, and destructuring-default contexts around recursive units.
All 136 accepted corpus files have no diagnostics or depth failure at maximum 128.

Removing the method guard preserves an acyclic parser-method graph after guarded vertices and recursive edges are removed.
Separate guards remain where each protects another independent cycle.
For example, a class guard covers recursive heritage, while a function guard covers nested functions and parameter initializers.
Parentheses and assignment, array, or loop guards protect distinct cycles.
Speculative parsing shares the same bound; it does not receive a separate counter or a recoverable failure.

| Repeated source form | Units per layer | Accepted bare source depth at maximum 128 |
|---|---:|---:|
| `(class{m(){return ...;}})` | 3: parentheses, class, function | 42 |
| `({m(){return ...;}})` | 3: parentheses, object, function | 42 |
| `export class C{m(){...}}` | 2: class, function | 64 |
| `(a=(a=...))` | 2: parentheses, assignment | 64 |
| `([([...])])` | 2: parentheses, array | 64 |
| `a:for(;;)...` | 2: label, for | 64 |
| All other recursive bare matrix forms | 1 | 128 |
| Iterative bare matrix forms | No accumulated recursive unit | No nesting rejection |

For a matrix form, accepted source depth is `floor((128 - outer units) / units per repeated layer)`.
The tests derive expected costs from the nested source constructs, independently of parser counters.

| Declaration wrapper | Additional guard units or applicability |
|---|---|
| Exported expression/type/binding/namespace | No additional units. |
| Exported statement container | One outer namespace unit. |
| Declare | Type aliases and namespaces only; other tested bodies/defaults cannot form valid ambient declarations. |
| Enum expression initializer | No additional units. |
| Enum initializer containing a statement/type/binding | One outer function unit. |
| Destructuring expression default | One outer array-binding unit. |
| Destructuring default containing a statement/type/binding | Two outer units: binding and function. |
| Repeated exported or enum-initialized function | One unit per function layer, without an additional container. |
| Repeated destructuring-default function | Two units per layer: binding and function; accepted source depth is 64. |

Type lookahead and speculative arrow heads remain guarded because their cycles precede AST construction.
Their entries contribute to the same active-unit count. Mixed source spellings can therefore exhaust the bound before 128 repeated source layers.
The fixture cost table describes these reviewed spellings, not every possible equivalent TypeScript spelling.

### Red and guard-removal evidence

Before production changes, the boundary test failed because parentheses at depth 129 returned no depth failure.
The unpatched 30,000-parenthesis witness then produced this actual output on a 1 MiB thread:

```text
running 1 test
thread '<unknown>' (28178661) has overflowed its stack
fatal runtime error: stack overflow, aborting
error: test failed, to rerun pass `-p hula-core --test parser_nesting`
(signal: 6, SIGABRT: process abort signal)
```

Cargo exited with status 101. The ignored witness was replaced by the guarded boundary and subprocess stack probes.
A separate 30,000-group regex witness also produced SIGABRT before the external-entry bound was added.

The review's declaration-dispatch inputs demonstrated why 1 MiB is insufficient despite a working depth guard.
Default tests now exercise them on 16 MiB. The parser does not claim that a guard unit bounds frame size.

The revision temporarily removed `parse_unary_expression`'s guard, then ran `other_recursive_paths` alone.
Actual output:

```text
thread 'other_recursive_paths' (28457275) panicked at crates/hula-core/tests/parser_nesting.rs:325:9:
recursive path typeof
test other_recursive_paths ... FAILED
Guard-removal exit: 101
```

The guard was restored in a `finally` block before further builds and measurements.

### Debug and release stack measurements

Platform: aarch64 macOS. Maximum nesting option: 128. Search resolution: 4,096 bytes.
Each candidate runs in a subprocess. Stack overflow terminates only the probe.
For compound forms and wrappers, probes use the deepest accepted complete source layer shown above.
Both builds measured all 196 applicable pairs. The ignored test prints every pair and each build's maximum.
Marks are minimum passing requested stack sizes, including thread-entry overhead; they are conservative stack-use proxies, not exact high-water measurements.
Rust and the OS round small requests to their minimum stack size.

| Review input at source depth 128 | Debug mark, bytes | Release mark, bytes |
|---|---:|---:|
| `"export enum E{a=function(){".repeat(128) + "}}".repeat(128)` | 1,200,128 | 626,688 |
| `"export const a=function(){".repeat(128) + "}".repeat(128)` | 1,167,360 | 610,304 |
| `"enum E{a=function(){".repeat(128) + "}}".repeat(128)` | 1,118,208 | 561,152 |
| `"export const a=()=>{".repeat(128) + "}".repeat(128)` | 937,984 | 446,464 |

| Wrapper family maximum | Debug mark, bytes | Debug worst form | Release mark, bytes | Release worst form |
|---|---:|---|---:|---|
| bare | 1,200,128 | `export_enum_functions/bare` | 626,688 | `export_enum_functions/bare` |
| export | 1,200,128 | `export_enum_functions/export` | 626,688 | `export_enum_functions/export` |
| declare | 626,688 | `namespaces/declare` | 249,856 | `namespaces/declare` |
| enum | 1,200,128 | `export_enum_functions/enum` | 626,688 | `export_enum_functions/enum` |
| default | 1,200,128 | `export_enum_functions/default` | 626,688 | `export_enum_functions/default` |

Global debug maximum: **1,200,128 bytes**, `export_enum_functions/bare`, at 128 function layers.
Global release maximum: **626,688 bytes**, the same input form.
Several outer wrappers tie these marks; the table reports the first maximum within each family.
Eight times the debug mark is 9,601,024 bytes, below the planned 16,777,216-byte reservation.
The debug mark does not exceed 2 MiB. The amended contract's 8x multiplier and 16 MiB floor remain unchanged.
These parser measurements inform the later compiler-thread slice; that slice must also account for checking, emission, and Luau compilation.

### Verification and limits

The requested offline, locked `parser_nesting` command passes: six tests, two ignored measurement helpers.
Default debug test runtime: 1.30 seconds (compilation excluded).
Debug and release ignored minimum-stack commands pass all 196 pairs.
`sh tools/gate.sh quick` passes all 12 CTest entries, formatting, and hygiene.
Clippy, tsc, Node compatibility witnesses, and the full release integration gate were not run.
Release parser stack measurement was run separately; it does not replace release integration verification.
No dependency, build configuration, public Hula ABI, Hula consumer, contract, or commit was changed by this revision.
Mapping `depth_exceeded` to `HULA_DEPTH_LIMIT` and providing the compiler thread remain outside this handoff's owned files.
The counter bounds recursive parsing, not AST depth produced by iterative chains.
Measurements apply to this platform and these source spellings; they are not a proof of a universal maximum frame cost.

### No speculative arrow parse for parameter defaults (2026-10-09)

Problem: `parse_possible_parenthesized_arrow_function_expression` speculatively parses `(a=` as arrow parameters.
In nested `(a=(a=...0))`, each failed attempt re-parses every inner level, and the arena keeps the discarded nodes.
Evidence: Hula's compiler memory meter measured 134,365,008 bytes for a 65,536-byte source of depth-63 statements.
Contract: `specs/contracts/p4/compiler-memory.md` and the Patch 2 paragraph of `specs/contracts/p2/pipeline.md`.

Change: when `max_nesting_depth` is set, `is_parenthesized_arrow_function_expression_worker` returns `False` for `(a=`.
The parser then parses the head as a parenthesized expression without speculation.
Hula rejects arrow parameter defaults, so `(a=1)=>0` remains a syntax error and no accepted source changes.
`(a?=`, `(a:T=`, and modifier heads still return `True`; `(a,` and `(a)` still speculate.
With `async`, `async (a=1)` parses as a call; `async (a=1)=>0` is a syntax error.
Without the nesting guard, upstream behavior is unchanged.

Arena bytes for one statement, measured with `Allocator::used_bytes` at maximum 128:

| Depth | Before | After |
|---:|---:|---:|
| 8 | 6,360 | 1,112 |
| 16 | 20,824 | 2,136 |
| 32 | 74,328 | 4,184 |
| 48 | 160,600 | 6,232 |
| 63 | 271,240 | 8,152 |

After the change each level adds 128 bytes.
The 65,536-byte Hula meter input peaks at 4,177,816 bytes in Release, down from 134,365,008.
The before column was measured on the base commit with the same probe; the probe asserted linear growth and failed there.
The crate's own unit tests need dev-dependencies that were not available offline; Hula's gate exercises the change.
