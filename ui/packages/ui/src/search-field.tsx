import { Button, InputGroup } from "@blueprintjs/core";

export interface SearchFieldProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  inputRef?: React.Ref<HTMLInputElement>;
  className?: string;
}

export const SearchField = ({
  value,
  onChange,
  placeholder = "Search...",
  inputRef,
  className,
}: SearchFieldProps) => (
  <InputGroup
    className={className}
    inputRef={inputRef}
    leftIcon="search"
    placeholder={placeholder}
    value={value}
    onChange={(event) => onChange(event.target.value)}
    fill
    rightElement={
      value ? (
        <Button icon="cross" minimal onClick={() => onChange("")} />
      ) : undefined
    }
  />
);
