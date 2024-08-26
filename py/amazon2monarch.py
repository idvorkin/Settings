import csv
import argparse
from datetime import datetime
import os
from rich import print

key_website = "Website"
key_order_date = "Order Date"
key_total_cost = "Total Owed"
key_name = "Product Name"
key_payment_type = "Payment Instrument Type"

whole_foods_value = "panda01"
amazon_com_value = "Amazon.com"


def convert(
    inputfile,
    outputfile,
    since_date,
    output_category,
    output_merchant_prefix,
    output_account,
    output_tags,
    output_notes_prefix,
):
    print("\n")
    print("Input CSV file is: ", inputfile.name)
    print("Output CSV will be written at: ", outputfile.name)

    if since_date is not None:
        print("Transactions after this date will be processed: ", since_date.date())

    print("\n")
    input("Press Enter to continue, or Control+C to abort...")

    amazon_transactions = 0
    whole_foods_transactions = 0
    zero_cost_transactions = 0

    csv_reader = csv.DictReader(inputfile, delimiter=",")

    output_field_names = [
        "Date",
        "Merchant",
        "Category",
        "Account",
        "Original Statement",
        "Notes",
        "Amount",
        "Tags",
    ]
    csv_writer = csv.DictWriter(outputfile, output_field_names)
    csv_writer.writeheader()

    for row in csv_reader:
        if row[key_website] == whole_foods_value:
            whole_foods_transactions += 1
        elif row[key_website] == amazon_com_value:
            transaction_date = strptime(row[key_order_date])
            if row[key_total_cost] == "0":
                zero_cost_transactions += 1
            elif (
                since_date is not None and since_date.date() <= transaction_date.date()
            ):
                amazon_transactions += 1
                csv_writer.writerow(
                    {
                        "Date": str(strptime(row[key_order_date]).date()),
                        "Merchant": output_merchant_prefix + row[key_name][0:31],
                        "Category": output_category,
                        "Account": output_account,
                        "Original Statement": row[key_name],
                        "Notes": output_notes_prefix + row[key_payment_type],
                        "Amount": "-" + row[key_total_cost],
                        "Tags": output_tags,
                    }
                )

    print("\n")
    if since_date is not None:
        print(
            f"Processed {amazon_transactions} Amazon.com transactions since {str(since_date.date())}."
        )
    else:
        print(f"Processed {amazon_transactions} Amazon.com transactions.")
    print(f"Ignored {whole_foods_transactions} Whole Foods transactions.")
    print(f"Ignored {zero_cost_transactions} transactions with $0 order amounts.")


def strptime(str):
    if "." not in str:
        return datetime.strptime(str, "%Y-%m-%dT%H:%M:%S%z")
    else:
        return datetime.strptime(str, "%Y-%m-%dT%H:%M:%S.%f%z")


def arg_date(str):
    try:
        return datetime.strptime(str, "%Y-%m-%d")
    except ValueError:
        msg = "not a valid date: {0!r}".format(str)
        raise argparse.ArgumentTypeError(msg)


parser = argparse.ArgumentParser(
    prog="amazon2monarch",
    description="This script converts Amazon Order History (Retail.OrderHistory.1.csv file) to a CSV that can be imported into Monarch.",
)
parser.add_argument(
    "-i",
    "--input_csv",
    required=True,
    type=argparse.FileType("r", encoding="utf-8-sig"),
    help="Path of the input CSV. This is usually the Retail.OrderHistory.1.csv file from your Amazon Data Request.",
)
parser.add_argument(
    "-o",
    "--output_csv",
    required=True,
    type=argparse.FileType("w"),
    help="Path of the output CSV. This is the file you will upload to Monarch.",
)
parser.add_argument(
    "-d",
    "--since_date",
    required=False,
    type=arg_date,
    help="Process transactions past since this date. If not provided, will process all transactions.",
)
parser.add_argument(
    "-oc",
    "--output_category",
    default="Uncategorized",
    help="Set the category of all transactions in the output file. Defaults to 'Uncategorized'.",
)
parser.add_argument(
    "-om",
    "--output_merchant_prefix",
    default="Amazon: ",
    help="Prefix each transactions with a text for ease of reading. Defaults to 'Amazon: '.",
)
parser.add_argument(
    "-oa",
    "--output_account",
    default="Amazon Gift Card Balance",
    help="Name of the account on Monarch for all the processed transactions. Defaults to 'Amazon Gift Card Balance'.",
)
parser.add_argument(
    "-ot",
    "--output_tags",
    default="Amazon Gift Card",
    help="Comma separated list of Tags on Monarch to be added to all transactions. Defaults to 'Amazon Gift Card'.",
)
parser.add_argument(
    "-on",
    "--output_notes_prefix",
    default="Payment Method: ",
    help="Prefix text to be added to transaction notes in the output files. Payment method is added by default, and the prefix 'Payment Method: ' is added.",
)

args = parser.parse_args()

try:
    convert(
        args.input_csv,
        args.output_csv,
        args.since_date,
        args.output_category,
        args.output_merchant_prefix,
        args.output_account,
        args.output_tags,
        args.output_notes_prefix,
    )
except Exception as e:
    print("\nUh Oh. Script failed because of the following reason: ")
    print(e.with_traceback)
    args.input_csv.close()
    args.output_csv.close()
    os.remove(args.output_csv.name)
finally:
    args.input_csv.close()
    args.output_csv.close()
    epilog = "Thank you for using this script. Hope it helped you. Cheers! 🍻"
    print("\n")
    print(epilog)
