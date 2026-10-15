/** An API failure: HTTP status plus the server's (or local engine's) message. */
export class ApiError extends Error {
  constructor(status, message) { super(message); this.status = status; }
}
