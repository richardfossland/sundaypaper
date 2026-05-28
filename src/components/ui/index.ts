// Barrel for the SundayPaper UI primitives. Built on Tailwind v4 design
// tokens (see src/styles/tokens.css). Theming flows through semantic CSS
// variables, so primitives never hard-code light/dark colors.
//
// Shipped in Phase 0.3: Button, Input, Textarea, Select, Badge, Card,
// Separator, Tabs, Dialog, Tooltip, PagePreview.
// Deferred until a feature needs them: Combobox, Sheet, Popover, Toast,
// DataTable.

export { Button, type ButtonProps } from "./button";
export { Input } from "./input";
export { Textarea } from "./textarea";
export { Select } from "./select";
export { Badge, type BadgeProps } from "./badge";
export {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "./card";
export { Separator } from "./separator";
export { Tabs, TabsList, TabsTrigger, TabsContent } from "./tabs";
export { Dialog } from "./dialog";
export { Tooltip } from "./tooltip";
export { PagePreview } from "./page-preview";
