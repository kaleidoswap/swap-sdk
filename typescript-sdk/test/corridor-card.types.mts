import {
  IntentsCorridor,
  type CorridorCard,
  type CorridorCardMarket,
  SwapClient,
} from "../dist/index.node.js";
import type { CorridorCard as BrowserCard } from "../dist/index.js";

const corridor = new IntentsCorridor(
  new SwapClient("https://maker.example/v2"),
);
const result: Promise<CorridorCard | null> = corridor.card();
const browserResult: Promise<BrowserCard | null> = result;
const direct = new SwapClient("https://maker.example/v2").corridorCard();
const directTyped: Promise<CorridorCard | null> = direct;
// @ts-expect-error direct WASM declarations must not widen the response to any
const untyped: Promise<string> = direct;
// @ts-expect-error absence must be handled
const required: Promise<CorridorCard> = corridor.card();
declare const market: CorridorCardMarket;
const limit: string = market.max_quote_amount;
// @ts-expect-error decimal amount strings are not rounded JS numbers
const rounded: number = market.max_quote_amount;
void [browserResult, directTyped, untyped, required, limit, rounded];
