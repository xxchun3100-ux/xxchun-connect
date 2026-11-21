// xxchun Connect has one relay: this shop's. (Upstream kept a list the person could add their own sites to.)
// The relay answers under more than one name, because some networks reset connections by the name asked for.
// The native side tries them in this order and writes the one that answered into the client's configuration;
// src-tauri/src/models.rs has the same list (BUILT_IN_ENTRIES), and a test compares the two.
export const ENTRIES = ["https://api.xxchunai.com", "https://api.xxchun.com"];
// Where people get their key and the tutorial. The shop itself is still served only under this name.
export const SHOP_SITE = "xxchun.com";
export interface Station {
  id: string;
  name: string;
  baseUrl: string;
}
export const DEFAULT_STATION: Station = {
  id: "xxchun",
  name: "xxchun",
  baseUrl: ENTRIES[0],
};
export const entryHost = (url: string) => url.replace(/^https?:\/\//, "");
export function stationAddress(station: Station): string {
  return entryHost(station.baseUrl);
}
// What the lock line says: the address that answered once there is one, otherwise every address that may be asked.
export const keyDestinations = (chosen: string) => (chosen ? entryHost(chosen) : ENTRIES.map(entryHost).join(" 或 "));
