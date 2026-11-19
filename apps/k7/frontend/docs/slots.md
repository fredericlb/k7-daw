# Slots, as k7 shows them

A frozen chain saved into a **slot** of this machine's engine plays with no page open once it is
enabled. The supervisor keeps the slots and the engine's rules are its own
(`engine/k7-supervisor/docs/slots.md`); this page is what k7 offers.

| choice | why |
|---|---|
| a chain is saved into a slot from the Frozen chains page, on one of the account's own chains (`../src/projects/FrozenChainsPage.tsx`) | that is where a chain is chosen; the bench is where it is tuned, and saving it into a slot is one more step |
| offered where this page's backend runs the machine's engine (the desktop app), and for every engine the account OWNS at this origin or through its server (`../src/settings/engineSlots.ts`) | a slot is what a machine plays with nobody attached, so only its owner sets it; a server's page has no engine of its own but may own a Pi |
| with more than one target, the slot button is a menu of them (`../src/projects/SlotButton.tsx`) | this machine first, then each owned engine by its host name |
| a remote engine's slot is compiled and sent by the server holding its tunnel (`apps/k7/backend-rs/docs/engine/slots.md`) | the chain and its files are the server's account library; the page only names the chain |
| the engines page lists each owned engine's slots, with the same actions (`../src/settings/RemoteSlotsSection.tsx`) | a machine that is not connected says so on its own card, the others still listed |
| a chain that cannot play live has no slot button, its reason in the tooltip | a slot plays a live input, as the bench does (`apps/k7/backend-rs/crates/k7-file-service/src/bench/mod.rs`) |
| the file service compiles it (`POST /account/slot`, `apps/k7/backend-rs/crates/k7-file-service/src/bench/slot.rs`), in a rig of its own beside the bench | the bench being tuned is never touched by a save |
| the page hands the compiled slot to its backend (`POST /api/local/engine/slots`), which stages it for the supervisor | the file service is the account's and may run elsewhere; the supervisor is this machine's |
| a slot's id is the chain's last name, folded (`slotIdOf`); saving the same chain again replaces its slot | "update the slot" is saving again, with nothing to pick |
| the engine page lists the slots, Enable or Disable on each and Remove (`../src/settings/SlotsSection.tsx`) | one enabled at most: enabling one disables the other |
| a chain row says when it is in a slot, and when that slot is the one playing | which chain the machine boots into should not need the engine page to find out |
| each slot row has a Footswitch button that asks for a CC number, empty for none (`../src/settings/askFootswitch.ts`), on this machine's slots and on owned engines' alike | a pedal board has no screen, so the switch is set where the slot is managed; a press toggles, so a latching pedal is set to momentary, and there is one mode to get wrong instead of two |
