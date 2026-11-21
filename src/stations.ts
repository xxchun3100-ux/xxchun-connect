// xxchun Connect has one relay: this shop's. (Upstream kept a list the person could add their own sites to.)
export interface Station {
  id: string;
  name: string;
  baseUrl: string;
}
export const DEFAULT_STATION: Station = {
  id: "xxchun",
  name: "xxchun",
  baseUrl: "https://api.xxchun.com",
};
export function stationAddress(station: Station): string {
  return station.baseUrl.replace(/^https?:\/\//, "");
}
