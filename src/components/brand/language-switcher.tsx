import { useTranslation } from "react-i18next";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { languageNames, type SupportedLanguage, supportedLanguages } from "@/i18n";

export function LanguageSwitcher() {
  const { i18n } = useTranslation();

  return (
    <Select
      value={i18n.language}
      onValueChange={(value) => {
        if (value) i18n.changeLanguage(value);
      }}
    >
      <SelectTrigger size="sm" className="w-36">
        <SelectValue>
          {(value: string | null) => (value ? languageNames[value as SupportedLanguage] : "")}
        </SelectValue>
      </SelectTrigger>
      <SelectContent>
        {supportedLanguages.map((lang) => (
          <SelectItem key={lang} value={lang}>
            {languageNames[lang]}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
