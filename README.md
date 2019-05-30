# README
This repository contains a rough work-in-progress rust implementation of variable length integer encoding from Wiredtiger.
The encoding also ensures the lexicographic order of resulting bytes is identical to integer ordering of the values.

Core algorithm for packing is implemented and is tested via quickcheck but much of ergonomics of using Rust IO traits
is missing as of now. 

As noted above original credit goes to Wiredtiger for the implementation.
