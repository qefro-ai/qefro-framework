import { ApiError } from "./api";
import { friendlyError } from "./friendlyError";

describe("friendlyError", () => {
  it("maps permission errors", () => {
    expect(friendlyError(new ApiError("forbidden", 403))).toMatch(/permission/i);
  });

  it("hides sql-like messages", () => {
    expect(friendlyError(new ApiError("select * from customers", 500))).toBe("Something went wrong.");
  });
});
