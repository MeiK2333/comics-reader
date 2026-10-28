import { definePlatform } from "../types";
import icon from "./icon.png";
import { ehentaiMessages } from "./messages";

export const ehentai = definePlatform<"ehentai">({
  id: "ehentai",
  messages: ehentaiMessages,
  icon,
  iconStyle: "plain",
  singleChapter: true,
});
