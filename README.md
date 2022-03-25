## zoltan
The goal of this project is to make it easy to generate debug symbols for reverse engineering on the fly.
It generates debug symbols from a combination of C source code annotated with patterns and executable binaries.

### usage
The first step is to define a function typedef in C with an IDA-style pattern, you can see some examples [here](#patterns).
Your C file will get parsed and any types you refer to in your typedefs (structs, enums etc.) will be stored in a debug symbol file later.
The next thing zoltan does is finding the patterns you specified in an executable provided by you. The underlying pattern search uses [a very fast SIMD-accelerated multi-string search algorithm](https://github.com/BurntSushi/aho-corasick), so it should complete very quickly.

Once you have your header file ready you can invoke Zoltan through command-line like this:
```powershell
zoltan '.\types.h' 'C:\Games\ELEX2\system\ELEX2.exe' '.\symbols'
```
This command will write the debug symbols to a file called `symbols`. Zoltan uses the DWARF format to encode them. The resulting symbol file can be loaded into RE tools like IDA for example (Edit->Plugins->Load DWARF file):

<img width="410" alt="zoltan" src="https://user-images.githubusercontent.com/11986158/159622437-3dc44968-3a49-4ace-9a2d-5d39e786e2f9.png">

Once you do this, you should be able to enjoy having all of your functions and data types visible in the decompiled code/instruction list.

### patterns
```C
// defines a pattern that matches function prologue exactly
/// @pattern 48 83 EC 30 48 8B 09 41 8B F1 41 8B E8 48 8B DA 48 85 C9 74 65
typedef void give_item(struct Object** target, struct Object** item, unsigned int quantity);

// defines a pattern with an offset to function prologue
/// @pattern 8B 0D ? ? ? ? BA 10 00 00 00 48 8B 0C C8 8B 04 0A 39 ? ? ? ? 01 7F 16
/// @offset 13
typedef struct FunctionRegistry* get_function_registry();

// defines a pattern that retrieves the function address from a relative CALL instruction 
/// @pattern E8 (fn:rel) 45 8B 86 70 01 00 00 33 C9 BA 05 00 00 00 C7 44 24 30 02 00 00 00
/// @eval fn
typedef struct Object* get_player();

// defines a pattern that retrieves the function address from the second slot of a virtual table
/// @pattern 46 58 00 00 00 00 48 8D 05 (vft:rel) 49 89 06 41 89 6E 60 49 8B C6 4C
/// @eval *(vft + 2)
typedef char* get_name(struct Object* npc);
```
