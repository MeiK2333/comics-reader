import { definePlatform } from "../types";
import icon from "./icon.svg";
import { nhentaiMessages } from "./messages";

export const nhentai = definePlatform<"nhentai">({
  id: "nhentai",
  messages: nhentaiMessages,
  icon,
  iconStyle: "dark-chip",
  singleChapter: true,
  numericIdDirectOpen: true,
});
