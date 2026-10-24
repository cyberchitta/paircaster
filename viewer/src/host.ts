import { connect } from "pair-ui/page";

// The pair-ui host in a browser, a framing host such as Shala, or nothing: a
// page opened as a plain file has no host.
export const host = await connect();
