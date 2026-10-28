import { definePlatform } from "../types";
import icon from "./icon.png";
import { pixivMessages } from "./messages";

export const pixiv = definePlatform<"pixiv">({
  id: "pixiv",
  messages: pixivMessages,
  icon,
  iconStyle: "plain",
  singleChapter: true,
  numericIdDirectOpen: true,
});
