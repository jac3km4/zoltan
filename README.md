# zoltan
The goal of this project is to make it easy to generate debug symbols and header files for reverse engineering on the fly.
It can generate debug symbols from a combination of C/C++ source code annotated with patterns and executable binaries.

## usage
The first step is to define a C/C++ function typedef with a byte pattern (similar to IDA), you can read more about it [here](#patterns).
The source file containing your typedefs will get parsed and any types you refer to (structs, enums etc.) can be stored in a debug file together with your functions by using the `--dwarf-output <DWARF>` [CLI option](#cli).
When Zoltan runs, it searches for your patterns in an executable provided by you and then uses the resolved function addresses to generate debug symbols that are compatible with your executable.
The underlying pattern search uses [a very fast SIMD-accelerated multi-string search algorithm](https://github.com/BurntSushi/aho-corasick), so it should generally complete very quickly.

Once you have your header file ready you can invoke Zoltan through command-line like this:
```powershell
zoltan-clang.exe '.\types.hpp' 'C:\Games\ELEX2\system\ELEX2.exe' -f 'std=c++20' --dwarf-output '.\dbg-symbols'
```
This command will write the debug symbols to a file called `dbg-symbols`. Zoltan uses the DWARF format to encode them. The resulting symbol file can be loaded into RE tools like IDA for example (Edit->Plugins->Load DWARF file).
Once you do this, you should be able to enjoy having all of your functions and data types visible in the decompiled code/instruction list.

The example above uses the clang frontend, you can read about other frontends [here](#frontends).

## cli
```
Zoltan Clang frontend for C/C++

Usage: <SOURCE> <EXE> [-o DWARF] [--c-output C] [--rust-output RUST] [[--strip-namespaces]] [[--eager-type-export]] -f FLAGS...

Available options:
    -o, --dwarf-output <DWARF>   DWARF file to write
        --c-output <C>           C header with offsets to write
        --rust-output <RUST>     Rust file with offsets to write
        --strip-namespaces       Strip namespaces from type names
        --eager-type-export      Export all types found in the sources
    -f, --compiler-flag <FLAGS>  Flags to pass to the compiler
    -h, --help                   Prints help information
```

## patterns
The patterns need to be written in comments prefixed by triple '`/`' immediately followed by a function typedef.
Zoltan supports standard IDA-style paterns:
```C
// defines a pattern that matches function prologue exactly
/// @pattern 48 83 EC 30 48 8B 09 41 8B F1 41 8B E8 48 8B DA 48 85 C9 74 65
typedef void give_item(struct Object** target, struct Object** item, unsigned int quantity);

// defines a pattern that matches the 6th (0-based) result out of 24
/// @pattern 57 48 83 EC 30 48 C7 44 24 20 FE FF FF FF 48 89 5C 24 48 48 8B
/// @nth 5/24
typedef void remove_item(struct Object** item);

// defines a pattern with an offset to function prologue
/// @pattern 8B 0D ? ? ? ? BA 10 00 00 00 48 8B 0C C8 8B 04 0A 39 ? ? ? ? 01 7F 16
/// @offset 13
typedef struct FunctionRegistry* get_function_registry();
```

Zoltan extends IDA-style patterns with matching groups, similar to regex. You can for example capture a relative offset to another function.
When using matching groups, you'll want to use an @eval parameter to inform zoltan how to calculate the resulting offset.
In the example below, we return a resolved address of a function:
```C
// defines a pattern that retrieves the function address from a relative CALL instruction 
/// @pattern E8 (fn:rel) 45 8B 86 70 01 00 00 33 C9 BA 05 00 00 00 C7 44 24 30 02 00 00 00
/// @eval fn
typedef struct Object* get_player();
```

The @eval parameter accepts expressions, giving you the flexibility to do things like matching on an access to a virtual table and pulling out specific functions from it:
```C

// defines a pattern that retrieves the function address from the second slot of a virtual table
/// @pattern 46 58 00 00 00 00 48 8D 05 (vft:rel) 49 89 06 41 89 6E 60 49 8B C6 4C
/// @eval *(vft + 2)
typedef char* get_name(struct Object* npc);
```

## generating headers
Zoltan can also generate headers with offsets of the resolved functions. You can do it using the `--c-output` and `--rust-output` options.
The generated files look like this:
```C
// This file has been generated by zoltan (https://github.com/jac3km4/zoltan)

#define GET_PLAYER_ADDR 0x40B820
#define GET_FUNCTION_REGISTRY_ADDR 0x867310
#define GIVE_ITEM_ADDR 0xB15170
```
Combined with your typedefs you can use them to invoke these functions at runtime:
```C
((get_player*)(IMAGE_BASE + GET_PLAYER_ADDR))()
```

## frontends
There are two frontends available:
- zoltan-saltwater
    - comes with a C compiler written in pure Rust ([saltwater](https://github.com/jac3km4/saltwater)), no external dependencies
    - very fast, good for prototyping
    - cannot compile C++
- zoltan-clang
    - uses libclang, so it can be used against complex C++ codebases leveraging modern standards
    - can be relatively slow because of the Clang parser
    - allows the use of C++ `using` type aliases instead of typedefs
    - requires libclang set up locally
        - on recent versions of windows you can do
            ```powershell
            winget install llvm
            $env:PATH += ";C:\Program Files\LLVM\bin"
            ```
        - the `PATH` change above isn't persistent, if you want to update `PATH` permanently you need to do it in the Control Panel
            
            ![PATH change](https://user-images.githubusercontent.com/11986158/163652902-b3a66b03-378e-40c5-bdcf-0a8363de3b06.png)
