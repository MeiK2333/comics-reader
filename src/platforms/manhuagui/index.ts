import { definePlatform } from "../types";
import icon from "./icon.svg";
import { manhuaguiMessages } from "./messages";

export const manhuagui = definePlatform<"manhuagui">({
  id: "manhuagui",
  messages: manhuaguiMessages,
  icon,
  iconStyle: "plain",
  singleChapter: false,
});
