import { definePlatform } from "../types";
import icon from "./icon.png";
import { jmMessages } from "./messages";

export const jm = definePlatform<"jm">({
  id: "jm",
  messages: jmMessages,
  icon,
  iconStyle: "plain",
  singleChapter: false,
});
