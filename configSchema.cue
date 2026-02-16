package budgetConfig

budgetName:          string
personalAccessToken: string
categoryGroupWatchList: {[string]: =~"^#[0-9a-fA-F]{6}$"}
resolution_date: null | (string & =~"^\\d{4}-\\d{2}-\\d{2}$")
showAllRows:     bool
outputFormat: "polars_print" | "csv_print" | {csv_output: string, visual_output?: _|_} | {visual_output: string, csv_output?: _|_}
