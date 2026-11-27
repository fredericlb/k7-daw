// searchRank.ts — hands the kit's search lists Rust's fuzzy scorer; imported once by appMain.
// One batched call per query ranks every row, so a list never scores its strings one by one.

import { fuzzyScores } from "@studio/k7-wasm/ui";
import { setSearchRank } from "@studio/ui/search-list";

setSearchRank(fuzzyScores);
