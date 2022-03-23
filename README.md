## zoltan
The aim of this project is to make it easy to generate debug symbols for your reverse engineering needs on the fly.
It generates debug symbols from a combination of C source code annotated with patterns and executable binaries.

### usage
The first step is to define a function typedef with an IDA-style pattern in a comment:
```C
/// @pattern FE FF FF FF 48 8B DA ? ? D1 48 8D 4C 24 30 E8 7E BD 11 00
typedef void testing(int a, long b);
```
You can also provide an offset if the pattern does not match the function's address exactly:
```C
/// @pattern FF FF 48 8B DA ? ? D1 48 8D 4C 24 30 E8 7E BD 11 00
/// @offset 2
typedef void testing(struct Vector3* a);
```
This file will get parsed and any types you refer to in your typedefs (structs, enums etc.) will be stored in a debug symbol file later.
The next thing Zoltan does is search for the patterns above in an executable provided by you. The underlying pattern search uses [a very fast SIMD-accelerated multi-string search algorithm](https://github.com/BurntSushi/aho-corasick), so it should complete very quickly.

Once you have your header file you can invoke Zoltan through command-line like this:
```powershell
zoltan '.\types.h' 'C:\Games\ELEX2\system\ELEX2.exe' '.\symbols'
```
This command will write the debug symbols to a file called `symbols`. Zoltan uses DWARF to encode symbols. The resulting symbol file can be loaded into RE tools like IDA for example (Edit->Plugins->Load DWARF file):

<img width="410" alt="zoltan" src="https://user-images.githubusercontent.com/11986158/159622437-3dc44968-3a49-4ace-9a2d-5d39e786e2f9.png">

Once you do this, you should be able to enjoy having all of your functions and data types visible in the decompiled code/instruction list.
