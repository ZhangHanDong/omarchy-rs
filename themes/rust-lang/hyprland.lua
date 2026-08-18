local active_border = {
  colors = { "rgba(f74c00ee)", "rgba(a84820ee)" },
  angle = 45,
}

hl.config({
  general = {
    col = {
      active_border = active_border,
      inactive_border = "rgba(49342baa)",
    },
  },
  group = {
    col = {
      border_active = active_border,
      border_inactive = "rgba(49342baa)",
    },
  },
})
