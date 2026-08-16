import jquery from "jquery";
import { runJqueryContract } from "../contract.js";

const $ = jquery?.fn ? jquery : jquery(window);
runJqueryContract($);
